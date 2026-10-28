//! Authored farm geometry data, corresponding to `data/farmPropGeometry.ts`.

#[cfg(feature = "flight-physics")]
use std::collections::{HashMap, HashSet};

#[cfg(feature = "flight-physics")]
use flight_sdk::{IndexBuffer, MeshGeometry, PrimitiveTopology, VertexSemantic};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FarmPropKind {
    Hay,
    Cow,
    Chickens,
}

pub const FARM_PROP_SCENE_SCALE: f32 = 0.018;
const CHICKEN_SCALE: f32 = 1.45;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FarmPropPoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FarmPropTriangleFilter {
    Aabb {
        max: FarmPropPoint,
        min: FarmPropPoint,
    },
    MaxX {
        value: f32,
    },
    NearestComponent {
        centers: &'static [FarmPropPoint],
        selected_index: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FarmPropPartSpec {
    pub filter: Option<&'static FarmPropTriangleFilter>,
    pub material_name: &'static str,
    pub node_name: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FarmPropSpec {
    pub center_x: f32,
    pub center_y: f32,
    pub center_z: f32,
    pub expected_source_extents: [f32; 3],
    pub expected_triangle_count: usize,
    pub label: &'static str,
    pub parts: &'static [FarmPropPartSpec],
    pub rotation_z: Option<f32>,
    pub scale_multiplier: Option<f32>,
}

const HAY_BALE_CENTERS: [FarmPropPoint; 6] = [
    FarmPropPoint {
        x: -25.51,
        y: 43.645,
        z: 1.005,
    },
    FarmPropPoint {
        x: -20.55,
        y: 39.51,
        z: 5.975,
    },
    FarmPropPoint {
        x: -25.72,
        y: 35.74,
        z: 5.975,
    },
    FarmPropPoint {
        x: -20.86,
        y: 31.76,
        z: 1.005,
    },
    FarmPropPoint {
        x: -20.14,
        y: 40.44,
        z: 1.005,
    },
    FarmPropPoint {
        x: -26.6,
        y: 34.91,
        z: 1.005,
    },
];
const FIRST_HAY_BALE_FILTER: FarmPropTriangleFilter = FarmPropTriangleFilter::NearestComponent {
    centers: &HAY_BALE_CENTERS,
    selected_index: 0,
};
const FRONT_COW_FILTER: FarmPropTriangleFilter = FarmPropTriangleFilter::MaxX { value: -20.0 };
const WHITE_CHICKEN_FILTER: FarmPropTriangleFilter = FarmPropTriangleFilter::Aabb {
    max: FarmPropPoint {
        x: -6.5,
        y: 16.2,
        z: 1.4,
    },
    min: FarmPropPoint {
        x: -8.3,
        y: 13.5,
        z: -1.2,
    },
};
const BROWN_CHICKEN_FILTER: FarmPropTriangleFilter = FarmPropTriangleFilter::Aabb {
    max: FarmPropPoint {
        x: -6.3,
        y: 26.5,
        z: 2.1,
    },
    min: FarmPropPoint {
        x: -8.5,
        y: 23.0,
        z: -1.6,
    },
};

const HAY_PARTS: [FarmPropPartSpec; 2] = [
    FarmPropPartSpec {
        filter: Some(&FIRST_HAY_BALE_FILTER),
        material_name: "HayBale2",
        node_name: "Object_22",
    },
    FarmPropPartSpec {
        filter: Some(&FIRST_HAY_BALE_FILTER),
        material_name: "HayBale",
        node_name: "Object_25",
    },
];
const COW_PARTS: [FarmPropPartSpec; 4] = [
    FarmPropPartSpec {
        filter: Some(&FRONT_COW_FILTER),
        material_name: "Cow1.001",
        node_name: "Object_30",
    },
    FarmPropPartSpec {
        filter: Some(&FRONT_COW_FILTER),
        material_name: "Cow3",
        node_name: "Object_34",
    },
    FarmPropPartSpec {
        filter: Some(&FRONT_COW_FILTER),
        material_name: "Cow1",
        node_name: "Object_41",
    },
    FarmPropPartSpec {
        filter: Some(&FRONT_COW_FILTER),
        material_name: "Cow2",
        node_name: "Object_42",
    },
];
const WHITE_CHICKEN_PARTS: [FarmPropPartSpec; 4] = [
    FarmPropPartSpec {
        filter: Some(&WHITE_CHICKEN_FILTER),
        material_name: "Hen4",
        node_name: "Object_29",
    },
    FarmPropPartSpec {
        filter: Some(&WHITE_CHICKEN_FILTER),
        material_name: "Hen3",
        node_name: "Object_33",
    },
    FarmPropPartSpec {
        filter: Some(&WHITE_CHICKEN_FILTER),
        material_name: "Hen2",
        node_name: "Object_38",
    },
    FarmPropPartSpec {
        filter: Some(&WHITE_CHICKEN_FILTER),
        material_name: "Hen_2",
        node_name: "Object_40",
    },
];
const BROWN_CHICKEN_PARTS: [FarmPropPartSpec; 4] = [
    FarmPropPartSpec {
        filter: Some(&BROWN_CHICKEN_FILTER),
        material_name: "material",
        node_name: "Object_27",
    },
    FarmPropPartSpec {
        filter: Some(&BROWN_CHICKEN_FILTER),
        material_name: "Hen4",
        node_name: "Object_29",
    },
    FarmPropPartSpec {
        filter: Some(&BROWN_CHICKEN_FILTER),
        material_name: "Hen3",
        node_name: "Object_33",
    },
    FarmPropPartSpec {
        filter: Some(&BROWN_CHICKEN_FILTER),
        material_name: "Hen2",
        node_name: "Object_38",
    },
];

const HAY_VARIANTS: [FarmPropSpec; 1] = [FarmPropSpec {
    center_x: -25.6,
    center_y: 43.645,
    center_z: 1.18,
    expected_source_extents: [5.08, 8.68, 5.6],
    expected_triangle_count: 124,
    label: "Hay bale",
    parts: &HAY_PARTS,
    rotation_z: Some(std::f32::consts::FRAC_PI_2),
    scale_multiplier: None,
}];
const COW_VARIANTS: [FarmPropSpec; 1] = [FarmPropSpec {
    center_x: -29.04,
    center_y: -51.48,
    center_z: 4.29,
    expected_source_extents: [8.76, 10.75, 8.19],
    expected_triangle_count: 3_780,
    label: "Cow",
    parts: &COW_PARTS,
    rotation_z: Some(-22.0 * std::f32::consts::PI / 180.0),
    scale_multiplier: Some(0.8),
}];
const CHICKEN_VARIANTS: [FarmPropSpec; 2] = [
    FarmPropSpec {
        center_x: -7.395,
        center_y: 14.865,
        center_z: 0.115,
        expected_source_extents: [1.49, 2.41, 2.32],
        expected_triangle_count: 1_312,
        label: "White hen",
        parts: &WHITE_CHICKEN_PARTS,
        rotation_z: None,
        scale_multiplier: Some(CHICKEN_SCALE),
    },
    FarmPropSpec {
        center_x: -7.42,
        center_y: 24.77,
        center_z: 0.245,
        expected_source_extents: [1.6, 3.25, 3.01],
        expected_triangle_count: 1_320,
        label: "Brown hen",
        parts: &BROWN_CHICKEN_PARTS,
        rotation_z: None,
        scale_multiplier: Some(0.756 * CHICKEN_SCALE),
    },
];

#[derive(Clone, Copy, Debug)]
pub struct FarmPropVariants {
    pub hay: &'static [FarmPropSpec],
    pub cow: &'static [FarmPropSpec],
    pub chickens: &'static [FarmPropSpec],
}

pub const FARM_PROP_VARIANTS: FarmPropVariants = FarmPropVariants {
    hay: &HAY_VARIANTS,
    cow: &COW_VARIANTS,
    chickens: &CHICKEN_VARIANTS,
};

impl FarmPropVariants {
    pub const fn get(&self, kind: FarmPropKind) -> &'static [FarmPropSpec] {
        match kind {
            FarmPropKind::Hay => self.hay,
            FarmPropKind::Cow => self.cow,
            FarmPropKind::Chickens => self.chickens,
        }
    }
}

pub fn get_random_farm_prop_variant_index(kind: FarmPropKind, draw: f64) -> usize {
    let variant_count = FARM_PROP_VARIANTS.get(kind).len();
    ((draw.max(0.0) * variant_count as f64).floor() as usize).min(variant_count - 1)
}

#[cfg(feature = "flight-physics")]
pub fn select_farm_prop_triangle_indices(
    source: &MeshGeometry,
    filter: &FarmPropTriangleFilter,
) -> Vec<u32> {
    if source.topology != PrimitiveTopology::TriangleList {
        return Vec::new();
    }
    let indices: Vec<u32> = match &source.indices {
        IndexBuffer::U16(indices) => indices
            .data
            .read()
            .unwrap()
            .iter()
            .copied()
            .map(u32::from)
            .collect(),
        IndexBuffer::U32(indices) => indices.data.read().unwrap().to_vec(),
        IndexBuffer::None => return Vec::new(),
    };
    let layout = source.layout.read().unwrap();
    let Some(position_attribute) = layout
        .attributes
        .iter()
        .find(|attribute| attribute.semantic == VertexSemantic::Position)
    else {
        return Vec::new();
    };
    let stride = layout.stride as usize / size_of::<f32>();
    let position_offset = position_attribute.byte_offset as usize / size_of::<f32>();
    let coordinate = |vertex_index: u32, axis: usize| -> f32 {
        source
            .vertices
            .get(vertex_index as usize * stride + position_offset + axis)
            .copied()
            .unwrap_or(0.0)
    };
    if let FarmPropTriangleFilter::NearestComponent {
        centers,
        selected_index,
    } = filter
    {
        return select_nearest_components(&indices, &coordinate, centers, *selected_index);
    }

    let mut selected_indices = Vec::new();
    for &[a, b, c] in indices.as_chunks::<3>().0 {
        let center = FarmPropPoint {
            x: (coordinate(a, 0) + coordinate(b, 0) + coordinate(c, 0)) / 3.0,
            y: (coordinate(a, 1) + coordinate(b, 1) + coordinate(c, 1)) / 3.0,
            z: (coordinate(a, 2) + coordinate(b, 2) + coordinate(c, 2)) / 3.0,
        };
        let matches = match filter {
            FarmPropTriangleFilter::MaxX { value } => center.x < *value,
            FarmPropTriangleFilter::Aabb { max, min } => {
                center.x >= min.x
                    && center.x <= max.x
                    && center.y >= min.y
                    && center.y <= max.y
                    && center.z >= min.z
                    && center.z <= max.z
            }
            FarmPropTriangleFilter::NearestComponent { .. } => unreachable!(),
        };
        if matches {
            selected_indices.extend([a, b, c]);
        }
    }
    selected_indices
}

#[cfg(feature = "flight-physics")]
fn select_nearest_components(
    indices: &[u32],
    coordinate: &impl Fn(u32, usize) -> f32,
    centers: &[FarmPropPoint],
    selected_index: usize,
) -> Vec<u32> {
    let triangle_count = indices.len() / 3;
    let mut parents: Vec<usize> = (0..triangle_count).collect();
    let mut triangle_by_position = HashMap::new();
    for triangle in 0..triangle_count {
        for corner in 0..3 {
            let vertex_index = indices[triangle * 3 + corner];
            let key = (
                js_round(coordinate(vertex_index, 0) * 1_000.0),
                js_round(coordinate(vertex_index, 1) * 1_000.0),
                js_round(coordinate(vertex_index, 2) * 1_000.0),
            );
            if let Some(matching_triangle) = triangle_by_position.get(&key).copied() {
                union_components(&mut parents, triangle, matching_triangle);
            } else {
                triangle_by_position.insert(key, triangle);
            }
        }
    }

    let mut component_bounds: HashMap<usize, ([f32; 3], [f32; 3])> = HashMap::new();
    for triangle in 0..triangle_count {
        let root = find_root(&mut parents, triangle);
        let (max, min) = component_bounds
            .entry(root)
            .or_insert(([f32::NEG_INFINITY; 3], [f32::INFINITY; 3]));
        for corner in 0..3 {
            let vertex_index = indices[triangle * 3 + corner];
            for axis in 0..3 {
                let value = coordinate(vertex_index, axis);
                min[axis] = min[axis].min(value);
                max[axis] = max[axis].max(value);
            }
        }
    }

    let mut selected_roots = HashSet::new();
    for (root, (max, min)) in component_bounds {
        let center = FarmPropPoint {
            x: (min[0] + max[0]) * 0.5,
            y: (min[1] + max[1]) * 0.5,
            z: (min[2] + max[2]) * 0.5,
        };
        if find_nearest_center(&center, centers) == selected_index {
            selected_roots.insert(root);
        }
    }

    let mut selected_indices = Vec::new();
    for triangle in 0..triangle_count {
        if selected_roots.contains(&find_root(&mut parents, triangle)) {
            selected_indices.extend_from_slice(&indices[triangle * 3..triangle * 3 + 3]);
        }
    }
    selected_indices
}

#[cfg(feature = "flight-physics")]
fn find_root(parents: &mut [usize], start: usize) -> usize {
    let mut root = start;
    while parents[root] != root {
        root = parents[root];
    }
    let mut current = start;
    while parents[current] != root {
        let next = parents[current];
        parents[current] = root;
        current = next;
    }
    root
}

#[cfg(feature = "flight-physics")]
fn union_components(parents: &mut [usize], a: usize, b: usize) {
    let root_a = find_root(parents, a);
    let root_b = find_root(parents, b);
    if root_a != root_b {
        parents[root_b] = root_a;
    }
}

#[cfg(feature = "flight-physics")]
fn find_nearest_center(center: &FarmPropPoint, candidates: &[FarmPropPoint]) -> usize {
    let mut nearest_index = 0;
    let mut nearest_distance_squared = f32::INFINITY;
    for (index, candidate) in candidates.iter().enumerate() {
        let dx = center.x - candidate.x;
        let dy = center.y - candidate.y;
        let dz = center.z - candidate.z;
        let distance_squared = dx * dx + dy * dy + dz * dz;
        if distance_squared < nearest_distance_squared {
            nearest_distance_squared = distance_squared;
            nearest_index = index;
        }
    }
    nearest_index
}

#[cfg(feature = "flight-physics")]
fn js_round(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}

#[cfg(test)]
mod tests {
    use super::{FarmPropKind, get_random_farm_prop_variant_index};

    #[test]
    fn chicken_variant_draws_match_the_typescript_boundaries() {
        assert_eq!(
            get_random_farm_prop_variant_index(FarmPropKind::Chickens, 0.0),
            0
        );
        assert_eq!(
            get_random_farm_prop_variant_index(FarmPropKind::Chickens, 0.5),
            1
        );
        assert_eq!(
            get_random_farm_prop_variant_index(FarmPropKind::Chickens, 1.0),
            1
        );
    }
}
