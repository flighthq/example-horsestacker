//! The round itself: phase machine, held piece, pile, physics, score, and result count-up.
//!
//! This follows `typescript-web/src/game/game.ts` declaration-for-declaration. Native ownership
//! lends dependencies through `GameDeps` rather than capturing them in a JavaScript closure.

use crate::native_storage::NativePreferences;
use crate::physics::{
    FINAL_SETTLE_SECONDS, GroundProfile, PASTURE_MAX_X, PASTURE_MIN_X, PASTURE_TOP_Y, PHYSICS_STEP,
    STACK_CONTACT_GROUND, STACK_CONTACT_PIECE, StackObjectKind, add_stack_object_body,
    create_horse_stack_world, does_stack_placement_overlap, get_ground_y, get_next_object_delay,
    get_random_stack_object_kind, get_settled_stack_height, get_stack_body_contacts,
    get_stack_body_half_width, get_stack_body_support_extent, get_stack_body_vertical_extent,
    get_stack_height_hands, get_stack_height_meters, get_stack_object_profile,
    get_stack_object_vertical_extent, get_supported_stack_height, remove_stack_object_body,
    step_horse_stack,
};
use crate::reduced_motion::prefers_reduced_motion;
use crate::{
    audio::AudioManager,
    data::{FarmPropKind, get_random_farm_prop_variant_index},
    scene::{
        CameraRig, LandingIndicator, ParticleEffects, SceneGraph, StackObjectVisuals,
        StackVisualHandle,
    },
};
use flight_sdk::{Ref, RigidBody2D, add_node_child, remove_node_children};

use super::{
    FIXED_STEP_LIMIT, GAME_DURATION_MS, GameMode, GamePhase, HORSE_DROP_ELAPSED_MS,
    HORSE_REST_ANGULAR, HORSE_REST_LINEAR, HORSE_REST_SECONDS, MAX_RESULT_COUNT_DURATION_MS,
    MIN_RESULT_COUNT_DURATION_MS, PLACEMENT_LIFT_STEP, STACK_BASE_Y, START_INPUT_GUARD_MS,
};

const BEST_HEIGHT_KEY_TIME: &str = "horse-stacker.best-height";
const BEST_HEIGHT_KEY_STEADY: &str = "horse-stacker.best-height.steady";

/// The piece currently hovering, waiting to be dropped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActiveStackObject {
    pub angle: f64,
    pub kind: StackObjectKind,
    pub variant_index: usize,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StackObjectSelection {
    pub kind: StackObjectKind,
    pub variant_index: usize,
}

/// A placed piece: body, visual handle, and dropped-horse bookkeeping.
pub struct StackedObject {
    pub body: Ref<RigidBody2D>,
    pub kind: StackObjectKind,
    pub lost: bool,
    pub dropped: bool,
    pub rest_seconds: f64,
    pub has_settled: bool,
    pub touched_ground: bool,
    pub collided_at: f64,
    pub handle: StackVisualHandle,
}

/// The TypeScript closure dependencies, borrowed for each native call.
pub struct GameDeps<'a> {
    pub audio: &'a mut AudioManager,
    pub camera_rig: &'a mut CameraRig,
    pub indicator: &'a mut LandingIndicator,
    pub particles: &'a mut ParticleEffects,
    pub scene_graph: &'a mut SceneGraph,
    pub visuals: &'a mut StackObjectVisuals,
    pub storage: &'a mut NativePreferences,
    pub announce: &'a mut dyn FnMut(&str),
}

/// Native counterpart of the TypeScript `Game` interface and `createGame` closure state.
pub struct Game {
    phase: GamePhase,
    placement_armed_at: f64,
    sway_clock: f64,
    ground_profile: Option<GroundProfile>,
    physics_world: crate::physics::HorseStackWorld,
    active_object: Option<ActiveStackObject>,
    stacked_objects: Vec<StackedObject>,
    objects_dropped: u32,
    aim_x: f64,
    aim_y: f64,
    nearby_bodies: Vec<Ref<RigidBody2D>>,
    aim_blocked: bool,
    aim_known: bool,
    last_aim_at: f64,
    next_object_at: f64,
    game_ends_at: f64,
    finish_at: f64,
    final_height: f64,
    cached_stack_height: f64,
    result_animation_start: f64,
    result_animation_duration: f64,
    result_hands: i64,
    result_hands_shown: i64,
    physics_accumulator: f64,
    last_impact_at: f64,
    mode: GameMode,
    peak_height: f64,
    horses_dropped: u32,
    queued: StackObjectSelection,
    best_meters: [Option<f64>; 2],
    record_before_round: Option<f64>,
    beat_the_record: bool,
    measurement_bodies: Vec<Ref<RigidBody2D>>,
    random_state: u64,
    now: f64,
}

pub fn create_game(storage: &NativePreferences) -> Game {
    let mut game = Game {
        phase: GamePhase::Loading,
        placement_armed_at: 0.0,
        sway_clock: 0.0,
        ground_profile: None,
        physics_world: create_horse_stack_world(None),
        active_object: None,
        stacked_objects: Vec::new(),
        objects_dropped: 0,
        aim_x: 0.0,
        aim_y: 0.0,
        nearby_bodies: Vec::new(),
        aim_blocked: false,
        aim_known: false,
        last_aim_at: 0.0,
        next_object_at: 0.0,
        game_ends_at: 0.0,
        finish_at: 0.0,
        final_height: 0.0,
        cached_stack_height: 0.0,
        result_animation_start: 0.0,
        result_animation_duration: 0.0,
        result_hands: 0,
        result_hands_shown: 0,
        physics_accumulator: 0.0,
        last_impact_at: 0.0,
        mode: GameMode::Time,
        peak_height: 0.0,
        horses_dropped: 0,
        queued: StackObjectSelection {
            kind: StackObjectKind::Horse,
            variant_index: 0,
        },
        best_meters: [
            read_best_meters(storage, GameMode::Time),
            read_best_meters(storage, GameMode::Steady),
        ],
        record_before_round: None,
        beat_the_record: false,
        measurement_bodies: Vec::new(),
        random_state: 0x88d1_5a47_c9e3_216b,
        now: 0.0,
    };
    game.queued = game.draw_piece();
    game
}

impl Game {
    pub const fn phase(&self) -> GamePhase {
        self.phase
    }
    pub const fn mode(&self) -> GameMode {
        self.mode
    }
    pub const fn is_running(&self) -> bool {
        matches!(self.phase, GamePhase::Playing | GamePhase::Settling)
    }
    pub const fn is_ready(&self) -> bool {
        !matches!(self.phase, GamePhase::Loading)
    }
    pub const fn objects_dropped(&self) -> u32 {
        self.objects_dropped
    }
    pub const fn final_height(&self) -> f64 {
        self.final_height
    }
    pub const fn displayed_height(&self) -> f64 {
        if matches!(self.phase, GamePhase::Finished) {
            self.final_height
        } else {
            self.cached_stack_height
        }
    }
    pub const fn peak_height(&self) -> f64 {
        if matches!(self.phase, GamePhase::Finished) {
            self.final_height
        } else {
            self.peak_height
        }
    }
    pub const fn next_kind(&self) -> StackObjectKind {
        self.queued.kind
    }
    pub const fn horses_dropped(&self) -> u32 {
        self.horses_dropped
    }
    pub const fn result_hands_shown(&self) -> i64 {
        self.result_hands_shown
    }
    pub const fn beat_the_record(&self) -> bool {
        self.beat_the_record
    }
    pub const fn record_before_round(&self) -> Option<f64> {
        self.record_before_round
    }

    pub fn count_progress(&self) -> f64 {
        if self.result_animation_start == 0.0 {
            return if self.phase == GamePhase::Finished {
                1.0
            } else {
                0.0
            };
        }
        ((self.now - self.result_animation_start) / self.result_animation_duration).clamp(0.0, 1.0)
    }

    pub fn time_up_progress(&self) -> f64 {
        if self.finish_at == 0.0 {
            return 0.0;
        }
        (1.0 - (self.finish_at - self.now) / (FINAL_SETTLE_SECONDS * 1_000.0)).clamp(0.0, 1.0) * 2.6
    }

    pub fn seconds_left(&self) -> f64 {
        if !self.game_ends_at.is_finite() {
            return 0.0;
        }
        ((self.game_ends_at - self.now) / 1_000.0).max(0.0)
    }

    pub fn mark_ready(&mut self, ground: Option<GroundProfile>) {
        self.ground_profile = ground;
        self.physics_world = create_horse_stack_world(self.ground_profile.as_ref());
        self.phase = GamePhase::Ready;
    }

    pub fn mark_lost(&mut self) {
        self.phase = GamePhase::Loading;
    }

    pub fn start_round(
        &mut self,
        deps: &mut GameDeps<'_>,
        next_mode: GameMode,
        now: f64,
        started_from: Option<f64>,
    ) {
        if !deps.visuals.is_ready() || self.phase == GamePhase::Loading {
            return;
        }
        self.mode = next_mode;
        deps.audio.start_round(now, self.mode);
        self.physics_world = create_horse_stack_world(self.ground_profile.as_ref());
        self.physics_accumulator = 0.0;
        self.active_object = None;
        self.stacked_objects.clear();
        self.objects_dropped = 0;
        self.aim_x = 0.0;
        self.aim_y = 0.0;
        self.aim_blocked = false;
        self.aim_known = false;
        deps.indicator.reset_teeter(now);
        self.last_aim_at = now;
        self.queued = self.draw_piece();
        self.next_object_at = 0.0;
        self.game_ends_at = if self.mode == GameMode::Steady {
            f64::INFINITY
        } else {
            now + GAME_DURATION_MS
        };
        self.finish_at = 0.0;
        self.final_height = 0.0;
        self.cached_stack_height = 0.0;
        self.peak_height = 0.0;
        self.horses_dropped = 0;
        self.result_animation_start = 0.0;
        self.result_animation_duration = 0.0;
        self.result_hands = 0;
        self.result_hands_shown = 0;
        self.beat_the_record = false;
        self.record_before_round = None;
        self.last_impact_at = 0.0;
        remove_node_children(&deps.scene_graph.stack_layer, None, None);
        deps.visuals.clear_pieces();
        remove_node_children(&deps.scene_graph.preview_layer, None, None);
        for node in deps.visuals.instanced_nodes() {
            add_node_child(&deps.scene_graph.stack_layer, node);
        }
        add_node_child(
            &deps.scene_graph.stack_layer,
            &deps.scene_graph.preview_layer,
        );
        deps.indicator.begin_round();
        deps.particles.reset();
        self.phase = GamePhase::Playing;
        deps.camera_rig.reset_height();
        self.placement_armed_at = started_from.unwrap_or(now) + START_INPUT_GUARD_MS;
        self.spawn_object(deps, now);
    }

    pub fn leave_round(&mut self, deps: &mut GameDeps<'_>) {
        if self.phase == GamePhase::Loading {
            return;
        }
        self.phase = GamePhase::Ready;
        self.active_object = None;
        deps.indicator.hide();
        deps.audio.leave_round();
        remove_node_children(&deps.scene_graph.stack_layer, None, None);
        deps.visuals.clear_pieces();
        remove_node_children(&deps.scene_graph.preview_layer, None, None);
        for node in deps.visuals.instanced_nodes() {
            add_node_child(&deps.scene_graph.stack_layer, node);
        }
        deps.particles.reset();
        self.cached_stack_height = 0.0;
        self.peak_height = 0.0;
        self.horses_dropped = 0;
        self.objects_dropped = 0;
        self.stacked_objects.clear();
        self.physics_world = create_horse_stack_world(self.ground_profile.as_ref());
        deps.camera_rig.reset_height();
    }

    pub fn update(&mut self, deps: &mut GameDeps<'_>, now: f64, delta_time: f64) {
        self.now = now;
        let was_running = self.is_running();
        if was_running {
            self.update_game(deps, now);
            self.sway_clock += delta_time * 1.6;
            self.step_game_physics(deps, now, delta_time);
            self.synchronize_stack_visuals(deps, now, delta_time);
            deps.visuals.flush();
            self.cached_stack_height = if self.phase == GamePhase::Finished {
                self.final_height
            } else {
                self.get_current_stack_height()
            };
            if self.phase != GamePhase::Finished {
                self.peak_height = self.peak_height.max(get_settled_stack_height(
                    &self.physics_world,
                    &self.measurement_bodies,
                    self.ground_profile.as_ref(),
                ));
            }
        }
        if self.phase == GamePhase::Finished && self.result_animation_start != 0.0 {
            self.update_result_animation(deps, now);
        }
    }

    pub fn reset_step_accumulator(&mut self) {
        self.physics_accumulator = 0.0;
    }

    pub fn place(&mut self, deps: &mut GameDeps<'_>, now: f64, input_at: Option<f64>) {
        if self.active_object.is_none() {
            return;
        }
        if input_at.unwrap_or(now) < self.placement_armed_at {
            return;
        }
        if now >= self.game_ends_at {
            self.begin_settling(deps, now);
            return;
        }
        self.update_active_stack_object(deps, now);
        self.commit_object_placement(deps, now);
    }

    pub fn aim_at(&mut self, deps: &mut GameDeps<'_>, x: f64, y: f64, now: f64) {
        if self.active_object.is_some() {
            self.set_aim(deps, x, y, now);
        }
    }

    pub fn nudge_aim(&mut self, deps: &mut GameDeps<'_>, dx: f64, dy: f64, now: f64) {
        if !self.aim_known {
            if let Some(active) = self.active_object {
                self.aim_x = 0.0;
                self.aim_y = self.get_landing_surface_y(0.0, active.kind)
                    + get_stack_object_vertical_extent(active.kind, 0.0);
            }
        }
        self.set_aim(deps, self.aim_x + dx, self.aim_y + dy, now);
    }

    fn get_aim_limits(&self) -> (f64, f64) {
        let kind = self
            .active_object
            .map_or(StackObjectKind::Horse, |active| active.kind);
        let inset = get_stack_object_profile(kind).half_width * 1.2;
        (PASTURE_MIN_X + inset, PASTURE_MAX_X - inset)
    }

    fn get_floor_y(&self, kind: StackObjectKind, angle: f64, x: f64) -> f64 {
        get_ground_y(self.ground_profile.as_ref(), x)
            + get_stack_object_vertical_extent(kind, angle)
    }

    fn is_placement_blocked(&mut self, kind: StackObjectKind, x: f64, y: f64, angle: f64) -> bool {
        let half_width = get_stack_object_profile(kind).half_width;
        let vertical = get_stack_object_vertical_extent(kind, angle);
        self.nearby_bodies.clear();
        for object in &self.stacked_objects {
            if object.lost {
                continue;
            }
            let body = object.body.read().unwrap();
            if (f64::from(body.x) - x).abs()
                >= half_width + get_stack_body_half_width(&self.physics_world.body_registry, &body)
            {
                continue;
            }
            if (f64::from(body.y) - y).abs()
                >= vertical
                    + get_stack_body_vertical_extent(&self.physics_world.body_registry, &body)
            {
                continue;
            }
            drop(body);
            self.nearby_bodies.push(object.body.clone());
        }
        !self.nearby_bodies.is_empty()
            && does_stack_placement_overlap(kind, x, y, angle, &self.nearby_bodies)
    }

    fn find_placeable_y(
        &mut self,
        kind: StackObjectKind,
        x: f64,
        from_y: f64,
        angle: f64,
    ) -> Option<f64> {
        if !self.is_placement_blocked(kind, x, from_y, angle) {
            return Some(from_y);
        }
        let extent = get_stack_object_vertical_extent(kind, angle);
        let on_top =
            (self.get_landing_surface_y(x, kind) + extent + PLACEMENT_LIFT_STEP).max(from_y);
        let step = extent * 0.25;
        let limit = on_top + extent * 2.0;
        let mut y = on_top;
        while y <= limit {
            if !self.is_placement_blocked(kind, x, y, angle) {
                return Some(y);
            }
            y += step;
        }
        let overhead = self.get_blocking_ceiling_y(kind, x)?;
        let clear = (overhead + extent + PLACEMENT_LIFT_STEP).max(on_top);
        (!self.is_placement_blocked(kind, x, clear, angle)).then_some(clear)
    }

    fn get_blocking_ceiling_y(&self, kind: StackObjectKind, x: f64) -> Option<f64> {
        let half_width = get_stack_object_profile(kind).half_width;
        let mut ceiling: Option<f64> = None;
        for object in &self.stacked_objects {
            if object.lost {
                continue;
            }
            let body = object.body.read().unwrap();
            if (f64::from(body.x) - x).abs()
                >= half_width + get_stack_body_half_width(&self.physics_world.body_registry, &body)
            {
                continue;
            }
            let top = f64::from(body.y)
                + get_stack_body_vertical_extent(&self.physics_world.body_registry, &body);
            ceiling = Some(ceiling.map_or(top, |value| value.max(top)));
        }
        ceiling
    }

    fn set_aim(&mut self, deps: &mut GameDeps<'_>, target_x: f64, target_y: f64, now: f64) {
        self.aim_known = true;
        let kind = self
            .active_object
            .map_or(StackObjectKind::Horse, |active| active.kind);
        let angle = self.active_object.map_or(0.0, |active| active.angle);
        let (min_x, max_x) = self.get_aim_limits();
        let next_x = target_x.clamp(min_x, max_x);
        let next_y = target_y.max(self.get_floor_y(kind, angle, next_x));
        let elapsed = ((now - self.last_aim_at) / 1_000.0).clamp(0.008, 0.08);
        deps.indicator.nudge((next_x - self.aim_x) / elapsed);
        self.aim_x = next_x;
        self.aim_y = next_y;
        self.last_aim_at = now;
    }

    fn get_landing_surface_y(&self, x: f64, kind: StackObjectKind) -> f64 {
        let mut surface_y = get_ground_y(self.ground_profile.as_ref(), x);
        let active_half_width = get_stack_object_profile(kind).half_width;
        for object in &self.stacked_objects {
            let body = object.body.read().unwrap();
            let horizontal_reach = active_half_width
                + get_stack_body_half_width(&self.physics_world.body_registry, &body);
            if object.lost
                || f64::from(body.y) < get_ground_y(self.ground_profile.as_ref(), f64::from(body.x))
                || (f64::from(body.x) - x).abs() > horizontal_reach * 0.92
                || body.velocity_y.abs() > 1.2
            {
                continue;
            }
            surface_y = surface_y.max(
                f64::from(body.y)
                    + get_stack_body_support_extent(&self.physics_world.body_registry, &body),
            );
        }
        surface_y
    }

    fn get_current_stack_height(&mut self) -> f64 {
        self.measurement_bodies.clear();
        self.measurement_bodies.extend(
            self.stacked_objects
                .iter()
                .filter(|object| !object.lost)
                .map(|object| object.body.clone()),
        );
        get_supported_stack_height(
            &self.physics_world,
            &self.measurement_bodies,
            self.ground_profile.as_ref(),
        )
    }

    fn update_active_stack_object(&mut self, deps: &mut GameDeps<'_>, now: f64) {
        let Some(mut current) = self.active_object else {
            return;
        };
        if !self.aim_known {
            deps.indicator.hide();
            return;
        }
        deps.indicator.step_teeter(now);
        let (min_x, max_x) = self.get_aim_limits();
        current.x = self.aim_x.clamp(min_x, max_x);
        current.angle = deps.indicator.angle();
        let held_y = self
            .aim_y
            .max(self.get_floor_y(current.kind, current.angle, self.aim_x));
        let placeable_y = self.find_placeable_y(current.kind, current.x, held_y, current.angle);
        self.aim_blocked = placeable_y.is_none();
        let Some(y) = placeable_y else {
            current.y = held_y;
            self.active_object = Some(current);
            deps.indicator.hide();
            return;
        };
        current.y = y;
        deps.indicator.update(
            deps.visuals,
            current.kind,
            current.x,
            current.y,
            current.angle,
            now,
        );
        self.active_object = Some(current);
    }

    fn spawn_object(&mut self, deps: &mut GameDeps<'_>, now: f64) {
        if self.phase != GamePhase::Playing || now >= self.game_ends_at {
            return;
        }
        deps.indicator.reset_teeter(now);
        self.last_aim_at = now;
        let selected = self.queued;
        self.queued = self.draw_piece();
        self.aim_y = self
            .aim_y
            .max(self.get_floor_y(selected.kind, 0.0, self.aim_x));
        self.aim_blocked = false;
        self.active_object = Some(ActiveStackObject {
            angle: 0.0,
            kind: selected.kind,
            variant_index: selected.variant_index,
            x: self.aim_x,
            y: self.aim_y,
        });
        let _ = deps
            .indicator
            .set_kind(deps.visuals, selected.kind, selected.variant_index);
        (deps.announce)(deps.visuals.label(selected.kind, selected.variant_index));
        self.update_active_stack_object(deps, now);
    }

    fn commit_object_placement(&mut self, deps: &mut GameDeps<'_>, now: f64) {
        let Some(current) = self.active_object else {
            return;
        };
        if self.phase != GamePhase::Playing
            || self.aim_blocked
            || self.is_placement_blocked(current.kind, current.x, current.y, current.angle)
        {
            return;
        }
        let body = add_stack_object_body(
            &mut self.physics_world,
            current.kind,
            current.x,
            current.y,
            current.angle,
        );
        {
            let mut body = body.write().unwrap();
            body.velocity_x = 0.0;
            body.velocity_y = 0.0;
            body.angular_velocity = 0.0;
        }
        let Ok(mut handle) = deps.visuals.add_piece(current.kind, current.variant_index) else {
            return;
        };
        deps.visuals.set_piece_transform(
            &mut handle,
            current.kind,
            current.x as f32,
            current.y as f32,
            current.angle as f32,
        );
        self.stacked_objects.push(StackedObject {
            body,
            kind: current.kind,
            lost: false,
            dropped: false,
            rest_seconds: 0.0,
            has_settled: false,
            touched_ground: false,
            collided_at: 0.0,
            handle,
        });
        self.active_object = None;
        deps.indicator.hide();
        self.objects_dropped += 1;
        deps.audio.play_stack_thud();
        if current.kind == StackObjectKind::Horse {
            deps.audio.maybe_play_horse_whinny(now);
        }
        self.next_object_at = now + get_next_object_delay(self.objects_dropped);
    }

    fn begin_settling(&mut self, deps: &mut GameDeps<'_>, now: f64) {
        self.phase = GamePhase::Settling;
        deps.audio.begin_result_count();
        self.active_object = None;
        self.finish_at = now + FINAL_SETTLE_SECONDS * 1_000.0;
        deps.indicator.hide();
    }

    fn record_final_height(&mut self, storage: &mut NativePreferences, meters: f64) {
        let index = mode_index(self.mode);
        let previous = self.best_meters[index];
        self.record_before_round = previous;
        self.beat_the_record = previous.is_some_and(|value| meters >= value + 0.01);
        if previous.is_some_and(|value| meters <= value) {
            return;
        }
        self.best_meters[index] = Some(meters);
        storage.set(best_height_key(self.mode), format!("{meters:.4}"));
    }

    fn finish_game(&mut self, deps: &mut GameDeps<'_>, now: f64) {
        self.phase = GamePhase::Finished;
        self.active_object = None;
        deps.indicator.hide();
        let current = self.get_current_stack_height();
        self.final_height = if self.mode == GameMode::Steady {
            self.peak_height.max(current)
        } else {
            current
        };
        self.cached_stack_height = self.final_height;
        self.record_final_height(deps.storage, get_stack_height_meters(self.final_height));
        self.result_hands = get_stack_height_hands(self.final_height);
        self.result_hands_shown = 0;
        deps.audio.arm_result_ticks(now);
        self.result_animation_start = now;
        self.result_animation_duration = if prefers_reduced_motion() {
            1.0
        } else {
            (1_800.0 + self.result_hands as f64 * 14.0)
                .clamp(MIN_RESULT_COUNT_DURATION_MS, MAX_RESULT_COUNT_DURATION_MS)
        };
        deps.scene_graph.indicator_light.write().unwrap().intensity = 0.0;
    }

    fn advance_horse_hands(&mut self, target_count: i64) -> bool {
        let previous = self.result_hands_shown;
        self.result_hands_shown = self
            .result_hands_shown
            .max(target_count.min(self.result_hands));
        self.result_hands_shown > previous
    }

    fn complete_result_animation(&mut self, deps: &mut GameDeps<'_>) {
        self.result_animation_start = 0.0;
        deps.audio.celebrate_result();
        self.advance_horse_hands(self.result_hands);
        deps.particles
            .burst_celebration((STACK_BASE_Y + 0.8_f64.max(self.final_height)) as f32);
    }

    fn update_result_animation(&mut self, deps: &mut GameDeps<'_>, now: f64) {
        if self.result_animation_start == 0.0 {
            return;
        }
        let progress =
            ((now - self.result_animation_start) / self.result_animation_duration).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - progress).powi(3);
        let hands = self
            .result_hands
            .min((self.result_hands as f64 * eased).floor() as i64);
        if self.advance_horse_hands(hands) {
            deps.audio.play_result_tick(now);
        }
        if progress >= 1.0 {
            self.complete_result_animation(deps);
        }
    }

    fn handle_physics_contacts(&mut self, deps: &mut GameDeps<'_>, now: f64) {
        if self.physics_world.physics.events.began.is_empty() || now - self.last_impact_at < 90.0 {
            return;
        }
        let contact = self.physics_world.physics.events.began[0].read().unwrap();
        let Some(point) = contact.points.first() else {
            return;
        };
        self.last_impact_at = now;
        deps.particles
            .burst_dust(point.x, STACK_BASE_Y as f32 + point.y);
    }

    fn step_game_physics(&mut self, deps: &mut GameDeps<'_>, now: f64, delta_time: f64) {
        self.physics_accumulator =
            (self.physics_accumulator + delta_time).min(PHYSICS_STEP * FIXED_STEP_LIMIT as f64);
        let mut steps = 0;
        while self.physics_accumulator >= PHYSICS_STEP && steps < FIXED_STEP_LIMIT {
            step_horse_stack(&mut self.physics_world);
            self.physics_accumulator -= PHYSICS_STEP;
            steps += 1;
            self.handle_physics_contacts(deps, now);
        }
    }

    fn synchronize_stack_visuals(&mut self, deps: &mut GameDeps<'_>, now: f64, delta_time: f64) {
        let mut retained = Vec::with_capacity(self.stacked_objects.len());
        for mut object in std::mem::take(&mut self.stacked_objects) {
            if object.lost {
                continue;
            }
            let (x, y, angle, vx, vy, angular_velocity, index) = {
                let body = object.body.read().unwrap();
                (
                    f64::from(body.x),
                    f64::from(body.y),
                    f64::from(body.angle),
                    f64::from(body.velocity_x),
                    f64::from(body.velocity_y),
                    f64::from(body.angular_velocity),
                    body.index,
                )
            };
            let speed = vx.hypot(vy);
            if speed < HORSE_REST_LINEAR && angular_velocity.abs() < HORSE_REST_ANGULAR {
                object.rest_seconds += delta_time;
                if object.rest_seconds >= HORSE_REST_SECONDS {
                    object.has_settled = true;
                }
            } else {
                object.rest_seconds = 0.0;
            }
            let contacts = get_stack_body_contacts(&self.physics_world, &object.body);
            if contacts & STACK_CONTACT_PIECE != 0 && object.collided_at == 0.0 {
                object.collided_at = now;
            }
            if !object.touched_ground && contacts & STACK_CONTACT_GROUND != 0 {
                object.touched_ground = true;
                if object.kind == StackObjectKind::Horse
                    && !object.dropped
                    && object.collided_at != 0.0
                    && (object.has_settled || now - object.collided_at > HORSE_DROP_ELAPSED_MS)
                {
                    object.dropped = true;
                    self.horses_dropped += 1;
                }
            }
            if y < -1.0 || x < PASTURE_MIN_X - 1.5 || x > PASTURE_MAX_X + 1.5 {
                object.lost = true;
                remove_stack_object_body(&mut self.physics_world, &object.body);
                if object.kind == StackObjectKind::Horse && !object.dropped {
                    self.horses_dropped += 1;
                }
                continue;
            }
            let carried = ((y - PASTURE_TOP_Y) / 0.9).clamp(0.0, 1.0);
            let sway = if prefers_reduced_motion() {
                0.0
            } else {
                carried * carried
            };
            let phase = self.sway_clock + f64::from(index) * 0.7;
            deps.visuals.set_piece_transform(
                &mut object.handle,
                object.kind,
                (x + phase.sin() * 0.0016 * sway) as f32,
                y as f32,
                (angle + (phase * 0.77 + 1.3).sin() * 0.010 * sway) as f32,
            );
            retained.push(object);
        }
        self.stacked_objects = retained;
    }

    fn update_game(&mut self, deps: &mut GameDeps<'_>, now: f64) {
        if self.phase == GamePhase::Playing {
            if now >= self.game_ends_at {
                self.begin_settling(deps, now);
                return;
            }
            if self.mode == GameMode::Steady && self.horses_dropped > 0 {
                deps.audio.begin_result_count();
                self.finish_game(deps, now);
                return;
            }
            if self.active_object.is_some() {
                self.update_active_stack_object(deps, now);
            } else if now >= self.next_object_at {
                self.spawn_object(deps, now);
            }
        } else if self.phase == GamePhase::Settling && now >= self.finish_at {
            self.finish_game(deps, now);
        }
    }

    fn draw_piece(&mut self) -> StackObjectSelection {
        let kind = get_random_stack_object_kind(self.next_random());
        let variant_index = match kind {
            StackObjectKind::Horse => 0,
            StackObjectKind::Hay => {
                get_random_farm_prop_variant_index(FarmPropKind::Hay, self.next_random())
            }
            StackObjectKind::Cow => {
                get_random_farm_prop_variant_index(FarmPropKind::Cow, self.next_random())
            }
            StackObjectKind::Chickens => {
                get_random_farm_prop_variant_index(FarmPropKind::Chickens, self.next_random())
            }
        };
        StackObjectSelection {
            kind,
            variant_index,
        }
    }

    fn next_random(&mut self) -> f64 {
        self.random_state ^= self.random_state << 13;
        self.random_state ^= self.random_state >> 7;
        self.random_state ^= self.random_state << 17;
        self.random_state as u32 as f64 / u32::MAX as f64
    }
}

fn best_height_key(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Time => BEST_HEIGHT_KEY_TIME,
        GameMode::Steady => BEST_HEIGHT_KEY_STEADY,
    }
}

fn mode_index(mode: GameMode) -> usize {
    match mode {
        GameMode::Time => 0,
        GameMode::Steady => 1,
    }
}

fn read_best_meters(storage: &NativePreferences, mode: GameMode) -> Option<f64> {
    storage
        .get(best_height_key(mode))?
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
}

#[cfg(test)]
mod tests {
    use super::{GameDeps, create_game};
    use crate::assets::asset_path;
    use crate::audio::create_audio_manager;
    use crate::game::{GameMode, GamePhase};
    use crate::native_storage::NativePreferences;
    use crate::scene::{
        create_camera_rig, create_landing_indicator, create_particle_effects, create_scene_graph,
        create_stack_object_visuals, extract_farm_prop_templates, load_gltf_scene,
    };

    #[test]
    fn complete_round_surface_starts_aims_places_and_leaves() {
        let mut storage = NativePreferences::memory();
        let mut game = create_game(&storage);
        let mut audio = create_audio_manager(asset_path("sounds"));
        let mut scene_graph = create_scene_graph();
        let mut particles = create_particle_effects(&scene_graph.root);
        let mut camera_rig = create_camera_rig();
        let mut visuals = create_stack_object_visuals();
        let farm = load_gltf_scene(asset_path("models/farm")).unwrap();
        let horse = load_gltf_scene(asset_path("models/horse")).unwrap();
        visuals.set_templates(extract_farm_prop_templates(&farm).unwrap(), horse);
        let mut indicator = create_landing_indicator(
            scene_graph.preview_layer.clone(),
            scene_graph.indicator_light.clone(),
        );
        let mut announcement = String::new();
        let mut announce = |text: &str| announcement = text.to_owned();

        game.mark_ready(None);
        {
            let mut deps = GameDeps {
                audio: &mut audio,
                camera_rig: &mut camera_rig,
                indicator: &mut indicator,
                particles: &mut particles,
                scene_graph: &mut scene_graph,
                visuals: &mut visuals,
                storage: &mut storage,
                announce: &mut announce,
            };
            game.start_round(&mut deps, GameMode::Time, 100.0, Some(100.0));
            game.aim_at(&mut deps, 0.0, 0.5, 500.0);
            game.place(&mut deps, 500.0, Some(500.0));
            game.leave_round(&mut deps);
        }

        assert!(!announcement.is_empty());
        assert_eq!(game.phase(), GamePhase::Ready);
        assert_eq!(game.objects_dropped(), 0);
    }
}
