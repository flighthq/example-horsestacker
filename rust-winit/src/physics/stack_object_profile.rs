use super::{PASTURE_TOP_Y, StackObjectKind};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StackObjectProfile {
    pub half_height: f64,
    pub half_width: f64,
    pub label: &'static str,
    pub support_half_height: f64,
}

pub const HORSE_SIZE_MULTIPLIER: f64 = 1.35;
pub const HORSE_HALF_WIDTH: f64 = 0.09 * HORSE_SIZE_MULTIPLIER;
pub const HORSE_HALF_HEIGHT: f64 = 0.0765 * HORSE_SIZE_MULTIPLIER;
pub const HORSE_BACK_RATIO: f64 = 0.447;
pub const HORSE_SUPPORT_HALF_HEIGHT: f64 = HORSE_HALF_HEIGHT * HORSE_BACK_RATIO;
pub const TYPICAL_HORSE_WITHERS_METERS: f64 = 1.55;
pub const HORSE_WITHERS_HEIGHT: f64 = HORSE_HALF_HEIGHT * (1.0 + HORSE_BACK_RATIO);
pub const METERS_PER_WORLD_UNIT: f64 = TYPICAL_HORSE_WITHERS_METERS / HORSE_WITHERS_HEIGHT;
pub const METERS_PER_HAND: f64 = 0.1016;

const HORSE_PROFILE: StackObjectProfile = StackObjectProfile {
    half_height: HORSE_HALF_HEIGHT,
    half_width: HORSE_HALF_WIDTH,
    label: "Horse",
    support_half_height: HORSE_SUPPORT_HALF_HEIGHT,
};
const HAY_PROFILE: StackObjectProfile = StackObjectProfile {
    half_height: 0.046,
    half_width: 0.0505,
    label: "Hay bale",
    support_half_height: 0.046,
};
const COW_PROFILE: StackObjectProfile = StackObjectProfile {
    half_height: 0.0886,
    half_width: 0.0591,
    label: "Cow",
    support_half_height: 0.0886,
};
const CHICKENS_PROFILE: StackObjectProfile = StackObjectProfile {
    half_height: 0.0309,
    half_width: 0.0309,
    label: "Chicken",
    support_half_height: 0.0309,
};

pub const fn stack_object_profile(kind: StackObjectKind) -> &'static StackObjectProfile {
    match kind {
        StackObjectKind::Horse => &HORSE_PROFILE,
        StackObjectKind::Hay => &HAY_PROFILE,
        StackObjectKind::Cow => &COW_PROFILE,
        StackObjectKind::Chickens => &CHICKENS_PROFILE,
    }
}

pub fn stack_object_vertical_extent(kind: StackObjectKind, angle: f64) -> f64 {
    let profile = stack_object_profile(kind);
    if kind == StackObjectKind::Chickens {
        return profile.half_height;
    }

    angle.cos().abs() * profile.half_height + angle.sin().abs() * profile.half_width
}

pub fn stack_object_support_extent(kind: StackObjectKind, angle: f64) -> f64 {
    let profile = stack_object_profile(kind);
    if kind == StackObjectKind::Chickens {
        return profile.support_half_height;
    }

    angle.cos().abs() * profile.support_half_height + angle.sin().abs() * profile.half_width
}

pub fn stack_height_meters(stack_top_y: f64) -> f64 {
    if stack_top_y <= 0.0 {
        return 0.0;
    }
    (stack_top_y - PASTURE_TOP_Y).max(0.0) * METERS_PER_WORLD_UNIT
}

pub fn stack_height_hands(stack_top_y: f64) -> i64 {
    (stack_height_meters(stack_top_y) / METERS_PER_HAND).round() as i64
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::{
        HORSE_HALF_WIDTH, METERS_PER_WORLD_UNIT, stack_height_hands, stack_height_meters,
        stack_object_vertical_extent,
    };
    use crate::physics::{PASTURE_TOP_Y, StackObjectKind};

    #[test]
    fn a_sideways_horse_uses_its_half_width_as_vertical_extent() {
        let extent = stack_object_vertical_extent(StackObjectKind::Horse, FRAC_PI_2);
        assert!((extent - HORSE_HALF_WIDTH).abs() < 1e-12);
    }

    #[test]
    fn score_is_measured_above_the_pasture() {
        let stack_top_y = 0.25;
        let expected = (stack_top_y - PASTURE_TOP_Y) * METERS_PER_WORLD_UNIT;
        assert!((stack_height_meters(stack_top_y) - expected).abs() < 1e-12);
        assert_eq!(
            stack_height_hands(stack_top_y),
            (expected / 0.1016).round() as i64
        );
    }
}
