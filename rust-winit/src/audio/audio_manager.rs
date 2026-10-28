//! Game audio orchestration, corresponding to `audio/audioManager.ts`.
//!
//! Flight's web host supplies decoded resources and an AudioContext. The native translation keeps
//! the same mixer/channel surface behind `AudioBackend`, allowing the winit host to provide the
//! device without changing game code.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::game::{
    AMBIENCE_FADE_AFTER_MS, AMBIENCE_FADE_MS, FINAL_WHINNY_CHANCE, GameMode,
    HORSE_PLACEMENT_WHINNY_CHANCE, HORSE_WHINNY_CUES, HORSE_WHINNY_INTERVAL_JITTER_MS,
    HORSE_WHINNY_MIN_INTERVAL_MS, RESULT_TICK_INTERVAL_MS,
};
use crate::native_storage::NativePreferences;
use crate::reduced_motion::prefers_reduced_motion;

pub type AudioChannel = u64;

#[derive(Clone, Copy, Debug)]
pub struct PlayAudioOptions {
    pub current_time: f64,
    pub gain: f32,
    pub loops: f64,
}

pub trait AudioBackend: Send {
    fn play(&mut self, resource: &Path, options: PlayAudioOptions) -> Option<AudioChannel>;
    fn stop(&mut self, channel: AudioChannel);
    fn set_gain(&mut self, channel: AudioChannel, gain: f32);
    fn set_muted(&mut self, channel: AudioChannel, muted: bool);
    fn fade_gain(&mut self, channel: AudioChannel, gain: f32, duration_ms: f64);
}

/// Default command sink used until a platform audio device is installed by the native host.
#[derive(Default)]
pub struct NativeAudioBackend {
    next_channel: AudioChannel,
    channels: HashMap<AudioChannel, PlayAudioOptions>,
}

impl AudioBackend for NativeAudioBackend {
    fn play(&mut self, _resource: &Path, options: PlayAudioOptions) -> Option<AudioChannel> {
        self.next_channel = self.next_channel.wrapping_add(1).max(1);
        self.channels.insert(self.next_channel, options);
        Some(self.next_channel)
    }

    fn stop(&mut self, channel: AudioChannel) {
        self.channels.remove(&channel);
    }

    fn set_gain(&mut self, channel: AudioChannel, gain: f32) {
        if let Some(options) = self.channels.get_mut(&channel) {
            options.gain = gain;
        }
    }

    fn set_muted(&mut self, _channel: AudioChannel, _muted: bool) {}

    fn fade_gain(&mut self, channel: AudioChannel, gain: f32, _duration_ms: f64) {
        self.set_gain(channel, gain);
    }
}

const SOUNDTRACK_SKIP_SECONDS: f64 = 2.0;
const MUTED_KEY: &str = "horse-stacker.muted";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum SoundName {
    Ambience,
    Fanfare,
    Soundtrack,
    Tada,
    Thud,
    Tick,
    Whinny,
}

const SOUND_FILES: [(SoundName, &str); 7] = [
    (SoundName::Ambience, "free-sound-1674978362.mp3"),
    (SoundName::Fanfare, "free-sound-1674977569.mp3"),
    (
        SoundName::Soundtrack,
        "Elijah_K - The Mountain's Happy Song.mp3",
    ),
    (SoundName::Tada, "free-sound-1674895520.mp3"),
    (SoundName::Thud, "free-sound-1674747349.mp3"),
    (SoundName::Tick, "free-sound-1674778893.mp3"),
    (SoundName::Whinny, "free-sound-effects-HORSE3.mp3"),
];

const fn gain(name: SoundName) -> f32 {
    match name {
        SoundName::Ambience => 0.16,
        SoundName::Fanfare => 0.46,
        SoundName::Soundtrack => 0.36,
        SoundName::Tada => 0.52,
        SoundName::Thud => 0.24,
        SoundName::Tick => 0.1,
        SoundName::Whinny => 0.22,
    }
}

pub struct AudioManager {
    backend: Box<dyn AudioBackend>,
    preferences: Arc<RwLock<NativePreferences>>,
    resources: HashMap<SoundName, PathBuf>,
    soundtrack_channel: Option<AudioChannel>,
    ambience_channel: Option<AudioChannel>,
    whinny_channel: Option<AudioChannel>,
    whinny_stops_at: Option<f64>,
    scheduled_whinny_at: Option<f64>,
    next_horse_whinny_at: f64,
    next_result_tick_at: f64,
    idle_since: f64,
    ambience_faded: bool,
    muted: bool,
    want_soundtrack: bool,
    want_ambience: bool,
    last_now: f64,
    random_state: u64,
}

pub fn create_audio_manager(sound_root: impl AsRef<Path>) -> AudioManager {
    AudioManager::with_backend(
        sound_root,
        Box::<NativeAudioBackend>::default(),
        Arc::new(RwLock::new(NativePreferences::memory())),
    )
}

impl AudioManager {
    pub fn with_backend(
        sound_root: impl AsRef<Path>,
        backend: Box<dyn AudioBackend>,
        preferences: Arc<RwLock<NativePreferences>>,
    ) -> Self {
        let sound_root = sound_root.as_ref();
        let resources = SOUND_FILES
            .into_iter()
            .map(|(name, file)| (name, sound_root.join(file)))
            .collect();
        let muted = read_muted(&preferences.read().unwrap());
        Self {
            backend,
            preferences,
            resources,
            soundtrack_channel: None,
            ambience_channel: None,
            whinny_channel: None,
            whinny_stops_at: None,
            scheduled_whinny_at: None,
            next_horse_whinny_at: 0.0,
            next_result_tick_at: 0.0,
            idle_since: 0.0,
            ambience_faded: false,
            muted,
            want_soundtrack: false,
            want_ambience: false,
            last_now: 0.0,
            random_state: 0x9c32_58ad_e013_77b1,
        }
    }

    pub const fn muted(&self) -> bool {
        self.muted
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        for channel in [
            self.soundtrack_channel,
            self.ambience_channel,
            self.whinny_channel,
        ]
        .into_iter()
        .flatten()
        {
            self.backend.set_muted(channel, muted);
        }
        self.preferences
            .write()
            .unwrap()
            .set(MUTED_KEY, if muted { "1" } else { "0" });
    }

    pub fn play_stack_thud(&mut self) {
        self.play(SoundName::Thud, 0.0, 0.0);
    }

    pub fn begin_result_count(&mut self) {
        self.want_soundtrack = false;
        self.soundtrack_channel = self.stop(self.soundtrack_channel);
        self.play(SoundName::Fanfare, 0.0, 0.0);
    }

    pub fn arm_result_ticks(&mut self, now: f64) {
        self.next_result_tick_at = now;
    }

    pub fn play_result_tick(&mut self, now: f64) {
        if prefers_reduced_motion() || now < self.next_result_tick_at {
            return;
        }
        self.next_result_tick_at = now + RESULT_TICK_INTERVAL_MS;
        self.play(SoundName::Tick, 0.0, 0.0);
    }

    pub fn celebrate_result(&mut self) {
        self.play(SoundName::Tada, 0.0, 0.0);
        if self.next_random() < FINAL_WHINNY_CHANCE {
            self.scheduled_whinny_at = Some(self.last_now + 180.0);
        }
    }

    pub fn maybe_play_horse_whinny(&mut self, now: f64) {
        if now < self.next_horse_whinny_at || self.next_random() >= HORSE_PLACEMENT_WHINNY_CHANCE {
            return;
        }
        self.play_whinny();
        self.next_horse_whinny_at = now
            + HORSE_WHINNY_MIN_INTERVAL_MS
            + self.next_random() * HORSE_WHINNY_INTERVAL_JITTER_MS;
    }

    pub fn start_round(&mut self, now: f64, mode: GameMode) {
        self.stop_whinny();
        self.next_horse_whinny_at = now
            + HORSE_WHINNY_MIN_INTERVAL_MS
            + self.next_random() * HORSE_WHINNY_INTERVAL_JITTER_MS;
        self.raise_ambience();
        self.soundtrack_channel = self.stop(self.soundtrack_channel);
        self.want_soundtrack = mode != GameMode::Steady;
        if self.want_soundtrack {
            self.soundtrack_channel =
                self.play(SoundName::Soundtrack, 0.0, SOUNDTRACK_SKIP_SECONDS);
        }
    }

    pub fn leave_round(&mut self) {
        self.want_soundtrack = false;
        self.soundtrack_channel = self.stop(self.soundtrack_channel);
        self.stop_whinny();
    }

    pub fn stop_all(&mut self) {
        self.want_soundtrack = false;
        self.want_ambience = false;
        self.soundtrack_channel = self.stop(self.soundtrack_channel);
        self.ambience_channel = self.stop(self.ambience_channel);
        self.stop_whinny();
    }

    pub fn update(&mut self, now: f64, playing: bool) {
        self.last_now = now;
        if self.whinny_stops_at.is_some_and(|at| now >= at) {
            self.whinny_stops_at = None;
            self.whinny_channel = self.stop(self.whinny_channel);
        }
        if self.scheduled_whinny_at.is_some_and(|at| now >= at) {
            self.scheduled_whinny_at = None;
            self.play_whinny();
        }
        self.reconcile_loops();
        if playing {
            if self.ambience_faded || self.idle_since != 0.0 {
                self.raise_ambience();
            }
            return;
        }
        if self.ambience_channel.is_none() || self.ambience_faded {
            return;
        }
        if self.idle_since == 0.0 {
            self.idle_since = now;
            return;
        }
        if now - self.idle_since < AMBIENCE_FADE_AFTER_MS {
            return;
        }
        self.ambience_faded = true;
        if let Some(channel) = self.ambience_channel {
            self.backend.fade_gain(channel, 0.0, AMBIENCE_FADE_MS);
        }
    }

    fn play(&mut self, name: SoundName, loops: f64, start_at: f64) -> Option<AudioChannel> {
        let resource = self.resources.get(&name)?;
        let channel = self.backend.play(
            resource,
            PlayAudioOptions {
                current_time: start_at,
                gain: gain(name),
                loops,
            },
        )?;
        if self.muted {
            self.backend.set_muted(channel, true);
        }
        Some(channel)
    }

    fn stop(&mut self, channel: Option<AudioChannel>) -> Option<AudioChannel> {
        if let Some(channel) = channel {
            self.backend.stop(channel);
        }
        None
    }

    fn stop_whinny(&mut self) {
        self.whinny_stops_at = None;
        self.scheduled_whinny_at = None;
        self.whinny_channel = self.stop(self.whinny_channel);
    }

    fn play_whinny(&mut self) {
        self.stop_whinny();
        let index = (self.next_random() * HORSE_WHINNY_CUES.len() as f64).floor() as usize;
        let Some(cue) = HORSE_WHINNY_CUES.get(index.min(HORSE_WHINNY_CUES.len() - 1)) else {
            return;
        };
        self.whinny_channel = self.play(SoundName::Whinny, 0.0, cue.start_seconds);
        self.whinny_stops_at = Some(self.last_now + cue.duration_seconds * 1_000.0);
    }

    fn raise_ambience(&mut self) {
        self.ambience_faded = false;
        self.idle_since = 0.0;
        self.want_ambience = true;
        if let Some(channel) = self.ambience_channel {
            self.backend.set_gain(channel, gain(SoundName::Ambience));
        } else {
            self.ambience_channel = self.play(SoundName::Ambience, f64::INFINITY, 0.0);
        }
    }

    fn reconcile_loops(&mut self) {
        if self.want_ambience && self.ambience_channel.is_none() {
            self.ambience_channel = self.play(SoundName::Ambience, f64::INFINITY, 0.0);
        }
        if self.want_soundtrack && self.soundtrack_channel.is_none() {
            self.soundtrack_channel =
                self.play(SoundName::Soundtrack, 0.0, SOUNDTRACK_SKIP_SECONDS);
        }
    }

    fn next_random(&mut self) -> f64 {
        self.random_state ^= self.random_state << 13;
        self.random_state ^= self.random_state >> 7;
        self.random_state ^= self.random_state << 17;
        self.random_state as f64 / u64::MAX as f64
    }
}

fn read_muted(preferences: &NativePreferences) -> bool {
    preferences.get(MUTED_KEY) == Some("1")
}
