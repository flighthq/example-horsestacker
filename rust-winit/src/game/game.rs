//! The round itself: phase, mode, queued pieces, clocks, placement cadence, and score.

use crate::physics::{FINAL_SETTLE_SECONDS, StackObjectKind, get_next_object_delay};
#[cfg(feature = "flight-physics")]
use flight_sdk::{Ref, RigidBody2D};

use super::{GAME_DURATION_MS, GameMode, GamePhase, START_INPUT_GUARD_MS};

/// The piece currently hovering, waiting to be dropped (`ActiveStackObject` in TypeScript).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActiveStackObject {
    pub angle: f64,
    pub kind: StackObjectKind,
    pub variant_index: usize,
    pub x: f64,
    pub y: f64,
}

/// A placed piece and its dropped-horse bookkeeping (`StackedObject` in TypeScript).
#[cfg(feature = "flight-physics")]
pub struct StackedObject {
    pub body: Ref<RigidBody2D>,
    pub kind: StackObjectKind,
    pub lost: bool,
    pub dropped: bool,
    pub rest_seconds: f64,
    pub has_settled: bool,
    pub touched_ground: bool,
    pub collided_at_ms: f64,
}

#[cfg(feature = "flight-physics")]
impl StackedObject {
    pub fn new(body: Ref<RigidBody2D>, kind: StackObjectKind) -> Self {
        Self {
            body,
            kind,
            lost: false,
            dropped: false,
            rest_seconds: 0.0,
            has_settled: false,
            touched_ground: false,
            collided_at_ms: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GameEvent {
    None,
    ObjectSpawned(StackObjectKind),
    BeganSettling,
    Finished { height: f64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlacementOutcome {
    NotPlaying,
    InputGuarded,
    NoActiveObject,
    RoundExpired,
    Placed(StackObjectKind),
}

/// The renderer-independent counterpart of TypeScript's `Game` interface and `createGame` state.
///
/// Physics owns the actual bodies and reports live and settled heights back to this type. Piece
/// draws are supplied by the host, which keeps random-number generation out of deterministic tests.
#[derive(Clone, Debug)]
pub struct Game {
    phase: GamePhase,
    mode: GameMode,
    active_object: Option<ActiveStackObject>,
    next_kind: StackObjectKind,
    objects_dropped: u32,
    horses_dropped: u32,
    placement_armed_at_ms: f64,
    next_object_at_ms: f64,
    game_ends_at_ms: f64,
    finish_at_ms: f64,
    displayed_height: f64,
    peak_height: f64,
    final_height: f64,
}

impl Game {
    fn new(first_queued_kind: StackObjectKind) -> Self {
        Self {
            phase: GamePhase::Loading,
            mode: GameMode::Time,
            active_object: None,
            next_kind: first_queued_kind,
            objects_dropped: 0,
            horses_dropped: 0,
            placement_armed_at_ms: 0.0,
            next_object_at_ms: 0.0,
            game_ends_at_ms: 0.0,
            finish_at_ms: 0.0,
            displayed_height: 0.0,
            peak_height: 0.0,
            final_height: 0.0,
        }
    }

    pub const fn phase(&self) -> GamePhase {
        self.phase
    }

    pub const fn mode(&self) -> GameMode {
        self.mode
    }

    pub const fn active_kind(&self) -> Option<StackObjectKind> {
        match self.active_object {
            Some(active) => Some(active.kind),
            None => None,
        }
    }

    pub const fn next_kind(&self) -> StackObjectKind {
        self.next_kind
    }

    pub const fn objects_dropped(&self) -> u32 {
        self.objects_dropped
    }

    pub const fn horses_dropped(&self) -> u32 {
        self.horses_dropped
    }

    pub const fn displayed_height(&self) -> f64 {
        if matches!(self.phase, GamePhase::Finished) {
            self.final_height
        } else {
            self.displayed_height
        }
    }

    pub const fn peak_height(&self) -> f64 {
        if matches!(self.phase, GamePhase::Finished) {
            self.final_height
        } else {
            self.peak_height
        }
    }

    pub const fn final_height(&self) -> f64 {
        self.final_height
    }

    pub const fn is_running(&self) -> bool {
        matches!(self.phase, GamePhase::Playing | GamePhase::Settling)
    }

    pub fn seconds_left_at(&self, now_ms: f64) -> f64 {
        if self.mode == GameMode::Steady || !self.is_running() {
            return 0.0;
        }
        ((self.game_ends_at_ms - now_ms) / 1_000.0).max(0.0)
    }

    pub fn mark_ready(&mut self) {
        self.phase = GamePhase::Ready;
    }

    pub fn mark_lost(&mut self) {
        self.phase = GamePhase::Loading;
        self.active_object = None;
    }

    /// Starts a round and returns the piece that should be materialized immediately.
    pub fn start_round(
        &mut self,
        mode: GameMode,
        now_ms: f64,
        first_kind: StackObjectKind,
        following_kind: StackObjectKind,
    ) -> Option<StackObjectKind> {
        if self.phase == GamePhase::Loading {
            return None;
        }

        self.mode = mode;
        self.phase = GamePhase::Playing;
        self.active_object = Some(ActiveStackObject {
            angle: 0.0,
            kind: first_kind,
            variant_index: 0,
            x: 0.0,
            y: 0.0,
        });
        self.next_kind = following_kind;
        self.objects_dropped = 0;
        self.horses_dropped = 0;
        self.placement_armed_at_ms = now_ms + START_INPUT_GUARD_MS;
        self.next_object_at_ms = 0.0;
        self.game_ends_at_ms = match mode {
            GameMode::Time => now_ms + GAME_DURATION_MS,
            GameMode::Steady => f64::INFINITY,
        };
        self.finish_at_ms = 0.0;
        self.displayed_height = 0.0;
        self.peak_height = 0.0;
        self.final_height = 0.0;
        self.active_kind()
    }

    pub fn leave_round(&mut self) {
        if self.phase == GamePhase::Loading {
            return;
        }
        self.phase = GamePhase::Ready;
        self.active_object = None;
        self.objects_dropped = 0;
        self.horses_dropped = 0;
        self.displayed_height = 0.0;
        self.peak_height = 0.0;
        self.final_height = 0.0;
    }

    pub fn place(&mut self, now_ms: f64, input_at_ms: f64) -> PlacementOutcome {
        if self.phase != GamePhase::Playing {
            return PlacementOutcome::NotPlaying;
        }
        if input_at_ms < self.placement_armed_at_ms {
            return PlacementOutcome::InputGuarded;
        }
        if now_ms >= self.game_ends_at_ms {
            self.begin_settling(now_ms);
            return PlacementOutcome::RoundExpired;
        }
        let Some(active) = self.active_object.take() else {
            return PlacementOutcome::NoActiveObject;
        };

        self.objects_dropped += 1;
        self.next_object_at_ms = now_ms + get_next_object_delay(self.objects_dropped);
        PlacementOutcome::Placed(active.kind)
    }

    pub fn record_horse_dropped(&mut self) {
        if self.phase == GamePhase::Playing {
            self.horses_dropped += 1;
        }
    }

    /// Advances the state machine. `draw_following` is evaluated only when a queued object spawns.
    pub fn update(
        &mut self,
        now_ms: f64,
        current_height: f64,
        settled_height: f64,
        draw_following: impl FnOnce() -> StackObjectKind,
    ) -> GameEvent {
        if self.is_running() {
            self.displayed_height = current_height.max(0.0);
            self.peak_height = self.peak_height.max(settled_height.max(0.0));
        }

        match self.phase {
            GamePhase::Playing if now_ms >= self.game_ends_at_ms => {
                self.begin_settling(now_ms);
                GameEvent::BeganSettling
            }
            GamePhase::Playing if self.mode == GameMode::Steady && self.horses_dropped > 0 => {
                GameEvent::Finished {
                    height: self.finish(current_height),
                }
            }
            GamePhase::Playing
                if self.active_object.is_none() && now_ms >= self.next_object_at_ms =>
            {
                let spawned = self.next_kind;
                self.active_object = Some(ActiveStackObject {
                    angle: 0.0,
                    kind: spawned,
                    variant_index: 0,
                    x: 0.0,
                    y: 0.0,
                });
                self.next_kind = draw_following();
                GameEvent::ObjectSpawned(spawned)
            }
            GamePhase::Settling if now_ms >= self.finish_at_ms => GameEvent::Finished {
                height: self.finish(current_height),
            },
            _ => GameEvent::None,
        }
    }

    fn begin_settling(&mut self, now_ms: f64) {
        self.phase = GamePhase::Settling;
        self.active_object = None;
        self.finish_at_ms = now_ms + FINAL_SETTLE_SECONDS * 1_000.0;
    }

    fn finish(&mut self, current_height: f64) -> f64 {
        self.phase = GamePhase::Finished;
        self.active_object = None;
        self.final_height = match self.mode {
            GameMode::Time => current_height.max(0.0),
            GameMode::Steady => self.peak_height.max(current_height.max(0.0)),
        };
        self.displayed_height = self.final_height;
        self.final_height
    }
}

/// Creates the game state, mirroring `createGame` while dependencies are still owned by the host.
pub fn create_game(first_queued_kind: StackObjectKind) -> Game {
    Game::new(first_queued_kind)
}

#[cfg(test)]
mod tests {
    use super::{Game, GameEvent, PlacementOutcome, create_game};
    use crate::game::{GAME_DURATION_MS, GameMode, GamePhase, START_INPUT_GUARD_MS};
    use crate::physics::StackObjectKind;

    #[test]
    fn a_round_cannot_start_before_assets_are_ready() {
        let mut round = create_game(StackObjectKind::Horse);

        assert_eq!(
            round.start_round(
                GameMode::Time,
                100.0,
                StackObjectKind::Horse,
                StackObjectKind::Hay
            ),
            None
        );
        assert_eq!(round.phase(), GamePhase::Loading);
    }

    #[test]
    fn starting_consumes_the_queue_and_arms_placement() {
        let mut round = ready_round(StackObjectKind::Horse);

        assert_eq!(
            round.start_round(
                GameMode::Time,
                100.0,
                StackObjectKind::Horse,
                StackObjectKind::Hay
            ),
            Some(StackObjectKind::Horse)
        );
        assert_eq!(round.active_kind(), Some(StackObjectKind::Horse));
        assert_eq!(round.next_kind(), StackObjectKind::Hay);
        assert_eq!(
            round.place(100.0, 100.0 + START_INPUT_GUARD_MS - 1.0),
            PlacementOutcome::InputGuarded
        );
        assert_eq!(
            round.place(500.0, 100.0 + START_INPUT_GUARD_MS),
            PlacementOutcome::Placed(StackObjectKind::Horse)
        );
        assert_eq!(round.objects_dropped(), 1);
    }

    #[test]
    fn placement_delay_spawns_the_queued_piece_and_draws_one_ahead() {
        let mut round = ready_round(StackObjectKind::Horse);
        round.start_round(
            GameMode::Time,
            0.0,
            StackObjectKind::Horse,
            StackObjectKind::Hay,
        );
        assert_eq!(
            round.place(START_INPUT_GUARD_MS, START_INPUT_GUARD_MS),
            PlacementOutcome::Placed(StackObjectKind::Horse)
        );

        assert_eq!(
            round.update(700.0, 0.2, 0.15, || StackObjectKind::Cow),
            GameEvent::ObjectSpawned(StackObjectKind::Hay)
        );
        assert_eq!(round.active_kind(), Some(StackObjectKind::Hay));
        assert_eq!(round.next_kind(), StackObjectKind::Cow);
        assert_eq!(round.displayed_height(), 0.2);
        assert_eq!(round.peak_height(), 0.15);
    }

    #[test]
    fn time_mode_settles_then_scores_the_standing_height() {
        let mut round = ready_round(StackObjectKind::Horse);
        round.start_round(
            GameMode::Time,
            1_000.0,
            StackObjectKind::Horse,
            StackObjectKind::Hay,
        );

        assert_eq!(
            round.update(1_000.0 + GAME_DURATION_MS, 1.4, 2.2, || {
                StackObjectKind::Cow
            }),
            GameEvent::BeganSettling
        );
        assert_eq!(round.phase(), GamePhase::Settling);
        assert_eq!(
            round.update(
                1_000.0 + GAME_DURATION_MS + crate::physics::FINAL_SETTLE_SECONDS * 1_000.0,
                0.9,
                2.2,
                || StackObjectKind::Cow
            ),
            GameEvent::Finished { height: 0.9 }
        );
        assert_eq!(round.final_height(), 0.9);
    }

    #[test]
    fn steady_mode_ends_on_one_dropped_horse_and_scores_the_peak() {
        let mut round = ready_round(StackObjectKind::Horse);
        round.start_round(
            GameMode::Steady,
            0.0,
            StackObjectKind::Horse,
            StackObjectKind::Hay,
        );
        round.update(1_000.0, 1.7, 1.6, || StackObjectKind::Cow);
        round.record_horse_dropped();

        assert_eq!(
            round.update(1_100.0, 0.4, 0.4, || StackObjectKind::Cow),
            GameEvent::Finished { height: 1.6 }
        );
        assert_eq!(round.horses_dropped(), 1);
        assert_eq!(round.phase(), GamePhase::Finished);
    }

    #[test]
    fn leaving_a_round_restores_the_ready_state() {
        let mut round = ready_round(StackObjectKind::Horse);
        round.start_round(
            GameMode::Steady,
            0.0,
            StackObjectKind::Horse,
            StackObjectKind::Hay,
        );
        round.leave_round();

        assert_eq!(round.phase(), GamePhase::Ready);
        assert_eq!(round.active_kind(), None);
        assert_eq!(round.objects_dropped(), 0);
    }

    fn ready_round(queued: StackObjectKind) -> Game {
        let mut round = create_game(queued);
        round.mark_ready();
        round
    }
}
