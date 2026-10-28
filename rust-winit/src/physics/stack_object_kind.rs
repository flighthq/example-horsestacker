#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StackObjectKind {
    Horse,
    Hay,
    Cow,
    Chickens,
}

impl StackObjectKind {
    pub const ALL: [Self; 4] = [Self::Horse, Self::Hay, Self::Cow, Self::Chickens];

    pub const fn weight(self) -> f64 {
        match self {
            Self::Horse => 0.30,
            Self::Hay => 0.50,
            Self::Cow => 0.05,
            Self::Chickens => 0.15,
        }
    }

    /// Select a kind from a random draw in the same way as the TypeScript implementation.
    pub fn from_unit_draw(draw: f64) -> Self {
        let total_weight: f64 = Self::ALL.into_iter().map(Self::weight).sum();
        let mut remaining = draw.clamp(0.0, 1.0 - f64::EPSILON) * total_weight;

        for kind in Self::ALL {
            if remaining < kind.weight() {
                return kind;
            }
            remaining -= kind.weight();
        }

        Self::Hay
    }
}

pub fn next_object_delay_ms(objects_dropped: u32) -> f64 {
    (210.0 - f64::from(objects_dropped) * 3.25).max(80.0)
}

pub fn pace_level(objects_dropped: u32) -> u32 {
    (1 + objects_dropped / 7).min(6)
}

#[cfg(test)]
mod tests {
    use super::{StackObjectKind, next_object_delay_ms, pace_level};

    #[test]
    fn weighted_boundaries_match_the_typescript_order() {
        assert_eq!(StackObjectKind::from_unit_draw(0.0), StackObjectKind::Horse);
        assert_eq!(StackObjectKind::from_unit_draw(0.30), StackObjectKind::Hay);
        assert_eq!(StackObjectKind::from_unit_draw(0.80), StackObjectKind::Cow);
        assert_eq!(
            StackObjectKind::from_unit_draw(0.85),
            StackObjectKind::Chickens
        );
        assert_eq!(
            StackObjectKind::from_unit_draw(1.0),
            StackObjectKind::Chickens
        );
    }

    #[test]
    fn pace_and_delay_are_bounded() {
        assert_eq!(pace_level(0), 1);
        assert_eq!(pace_level(10_000), 6);
        assert_eq!(next_object_delay_ms(0), 210.0);
        assert_eq!(next_object_delay_ms(10_000), 80.0);
    }
}
