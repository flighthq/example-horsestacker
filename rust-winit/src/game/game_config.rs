use crate::physics::{HORSE_HALF_HEIGHT, HORSE_SIZE_MULTIPLIER};

pub const STACK_BASE_Y: f64 = -0.057;
pub const STACK_X: f64 = 1.55;
pub const STACK_Z: f64 = -2.15;
pub const HORSE_SCALE: f64 = 0.00279 * HORSE_SIZE_MULTIPLIER;
pub const HORSE_VISUAL_CENTER_Y: f64 = 0.07875 * HORSE_SIZE_MULTIPLIER;
pub const LANDING_PREVIEW_LIFT: f64 = HORSE_HALF_HEIGHT * 1.2;

pub const CAMERA_PILE_FILL: f64 = 0.8;
pub const CAMERA_PILE_FILL_AT_HEIGHT: f64 = 0.56;
pub const CAMERA_TOP_BIAS: f64 = 0.16;
pub const CAMERA_EARLY_HEADROOM: f64 = HORSE_HALF_HEIGHT * 2.0;
pub const CAMERA_MIN_DISTANCE: f64 = 1.7;
pub const CAMERA_MAX_DISTANCE: f64 = 7.5;
pub const CAMERA_HEIGHT_DEADBAND: f64 = 0.008;
pub const CAMERA_HEIGHT_RISE_RATE: f64 = 1.1;
pub const CAMERA_HEIGHT_FALL_RATE: f64 = 0.14;
pub const CAMERA_HEIGHT_COLLAPSE_RATE: f64 = 1.6;

pub const WINDMILL_RADIANS_PER_SECOND: f64 = std::f64::consts::TAU / 14.0;
pub const START_INPUT_GUARD_MS: f64 = 400.0;
pub const GAME_DURATION_MS: f64 = 30_000.0;
pub const MIN_RESULT_COUNT_DURATION_MS: f64 = 2_200.0;
pub const MAX_RESULT_COUNT_DURATION_MS: f64 = 4_000.0;
pub const RESULT_TICK_INTERVAL_MS: f64 = 32.0;
pub const AMBIENCE_FADE_AFTER_MS: f64 = 45_000.0;
pub const AMBIENCE_FADE_MS: f64 = 6_000.0;
pub const HORSE_WHINNY_MIN_INTERVAL_MS: f64 = 9_000.0;
pub const HORSE_WHINNY_INTERVAL_JITTER_MS: f64 = 6_000.0;
pub const FINAL_WHINNY_CHANCE: f64 = 0.28;
pub const HORSE_PLACEMENT_WHINNY_CHANCE: f64 = 0.3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WhinnyCue {
    pub start_seconds: f64,
    pub duration_seconds: f64,
}

pub const HORSE_WHINNY_CUES: [WhinnyCue; 4] = [
    WhinnyCue {
        start_seconds: 0.08,
        duration_seconds: 2.1,
    },
    WhinnyCue {
        start_seconds: 3.7,
        duration_seconds: 1.2,
    },
    WhinnyCue {
        start_seconds: 5.7,
        duration_seconds: 1.7,
    },
    WhinnyCue {
        start_seconds: 8.82,
        duration_seconds: 1.25,
    },
];

pub const INDICATOR_SPRING: f64 = 22.0;
pub const INDICATOR_DAMPING: f64 = 6.2;
pub const INDICATOR_MAX_ANGLE: f64 = 0.65;
pub const INDICATOR_MAX_SPIN: f64 = 5.5;
pub const FIXED_STEP_LIMIT: usize = 6;

pub const HORSE_REST_LINEAR: f64 = 0.05;
pub const HORSE_REST_ANGULAR: f64 = 0.2;
pub const HORSE_REST_SECONDS: f64 = 0.12;
pub const PLACEMENT_LIFT_STEP: f64 = 0.002;
pub const HORSE_DROP_ELAPSED_MS: f64 = 500.0;
