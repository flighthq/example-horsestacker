//! Sampling the modelled ground under the play plane, corresponding to
//! `scene/terrainProfile.ts`.

use std::sync::Arc;

use flight_sdk::node::contract::NodeRevisionState;
use flight_sdk::{
    Mesh, Scene3D, Scene3DPickOptions, create_ray3d, create_scene3d_hit, create_vector3,
    pick_scene3d_with_ray3d, set_ray3d,
};

use crate::game::{STACK_BASE_Y, STACK_X, STACK_Z};
use crate::physics::{GroundProfile, PASTURE_MIN_X, PASTURE_WIDTH};

#[derive(Clone, Debug, PartialEq)]
pub struct TerrainProfile {
    pub min_x: f64,
    pub step: f64,
    pub heights: Vec<f64>,
}

impl From<TerrainProfile> for GroundProfile {
    fn from(profile: TerrainProfile) -> Self {
        Self {
            min_x: profile.min_x,
            step: profile.step,
            heights: profile.heights,
        }
    }
}

const GROUND_MATERIALS: [&str; 2] = ["Ground", "Ground2"];
const SAMPLE_COUNT: usize = 80;
const RAY_START_Y: f32 = 4.0;
const MIN_WALKABLE_NORMAL_Y: f32 = 0.5;

fn is_ground_mesh(mesh: &Mesh) -> bool {
    mesh.materials.read().unwrap().iter().any(|material| {
        material
            .as_ref()
            .and_then(|material| material.read().unwrap().material_fields().name.clone())
            .is_some_and(|name| GROUND_MATERIALS.contains(&name.as_str()))
    })
}

pub fn sample_farm_terrain(farm: &Scene3D) -> Option<TerrainProfile> {
    let min_x = PASTURE_MIN_X;
    let step = PASTURE_WIDTH / (SAMPLE_COUNT - 1) as f64;
    let mut heights = vec![f64::NAN; SAMPLE_COUNT];
    let mut ray = create_ray3d(0.0, 0.0, 0.0, 0.0, -1.0, 0.0);
    let mut hit = create_scene3d_hit();
    let mut origin = create_vector3(STACK_X as f32, RAY_START_Y, 0.0);
    let down = create_vector3(0.0, -1.0, 0.0);
    let options = Scene3DPickOptions {
        cull_backfaces: Some(false),
        max_distance: None,
        predicate: Some(Arc::new(is_ground_mesh)),
    };
    let mut revisions = NodeRevisionState::default();
    for (sample, height) in heights.iter_mut().enumerate() {
        let physics_x = min_x + step * sample as f64;
        origin.z = STACK_Z as f32 - physics_x as f32;
        set_ray3d(&mut ray, &origin, &down);
        if !pick_scene3d_with_ray3d(
            &mut hit,
            &farm.root,
            &ray,
            Some(&options),
            &mut revisions,
        ) {
            continue;
        }
        let normal_length =
            (hit.normal_x * hit.normal_x + hit.normal_y * hit.normal_y + hit.normal_z * hit.normal_z)
                .sqrt();
        if normal_length < 1e-12
            || hit.normal_y.abs() / normal_length < MIN_WALKABLE_NORMAL_Y
        {
            continue;
        }
        *height = f64::from(hit.point_y) - STACK_BASE_Y;
    }
    fill_gaps(&mut heights).then_some(TerrainProfile {
        heights,
        min_x,
        step,
    })
}

fn fill_gaps(heights: &mut [f64]) -> bool {
    let Some(first_covered) = heights.iter().position(|height| !height.is_nan()) else {
        return false;
    };
    for index in (0..first_covered).rev() {
        heights[index] = heights[index + 1];
    }
    for index in first_covered + 1..heights.len() {
        if heights[index].is_nan() {
            heights[index] = heights[index - 1];
        }
    }
    true
}
