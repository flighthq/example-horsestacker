#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StackObjectKind {
    Horse,
    Hay,
    Cow,
    Chickens,
}

pub const STACK_OBJECT_KINDS: [StackObjectKind; 4] = [
    StackObjectKind::Horse,
    StackObjectKind::Hay,
    StackObjectKind::Cow,
    StackObjectKind::Chickens,
];
pub const STACK_OBJECT_WEIGHTS: [f64; 4] = [0.30, 0.50, 0.05, 0.15];

/// Selects a kind from a unit random draw, mirroring `getRandomStackObjectKind(random)`.
pub fn get_random_stack_object_kind(draw: f64) -> StackObjectKind {
    let total_weight: f64 = STACK_OBJECT_WEIGHTS.iter().sum();
    let mut remaining = draw.clamp(0.0, 1.0 - f64::EPSILON) * total_weight;

    for (kind, weight) in STACK_OBJECT_KINDS.into_iter().zip(STACK_OBJECT_WEIGHTS) {
        if remaining < weight {
            return kind;
        }
        remaining -= weight;
    }

    StackObjectKind::Hay
}

pub fn get_next_object_delay(objects_dropped: u32) -> f64 {
    (210.0 - f64::from(objects_dropped) * 3.25).max(80.0)
}

pub fn get_pace_level(objects_dropped: u32) -> u32 {
    (1 + objects_dropped / 7).min(6)
}

#[cfg(test)]
mod tests {
    use super::{
        StackObjectKind, get_next_object_delay, get_pace_level, get_random_stack_object_kind,
    };

    #[test]
    fn weighted_boundaries_match_the_typescript_order() {
        assert_eq!(get_random_stack_object_kind(0.0), StackObjectKind::Horse);
        assert_eq!(get_random_stack_object_kind(0.30), StackObjectKind::Hay);
        assert_eq!(get_random_stack_object_kind(0.80), StackObjectKind::Cow);
        assert_eq!(
            get_random_stack_object_kind(0.85),
            StackObjectKind::Chickens
        );
        assert_eq!(get_random_stack_object_kind(1.0), StackObjectKind::Chickens);
    }

    #[test]
    fn pace_and_delay_are_bounded() {
        assert_eq!(get_pace_level(0), 1);
        assert_eq!(get_pace_level(10_000), 6);
        assert_eq!(get_next_object_delay(0), 210.0);
        assert_eq!(get_next_object_delay(10_000), 80.0);
    }
}
