//! Turning a screen point into a physics-plane point, corresponding to
//! `scene/playPlane.ts`.

use flight_sdk::{
    Camera3D, create_plane, create_ray3d, create_vector3, get_camera_3d_screen_to_world_ray,
    get_ray3d_point_at, intersect_ray3d_plane,
};

use crate::game::{STACK_BASE_Y, STACK_X, STACK_Z};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayPlanePoint {
    pub x: f32,
    pub y: f32,
}

/// Writes the physics-plane coordinates under an NDC point into `out`.
pub fn unproject_to_play_plane(
    out: &mut PlayPlanePoint,
    camera: &Camera3D,
    ndc_x: f32,
    ndc_y: f32,
    aspect: f32,
) -> bool {
    let mut ray = create_ray3d(0.0, 0.0, 0.0, 0.0, 0.0, -1.0);
    if !get_camera_3d_screen_to_world_ray(&mut ray, camera, ndc_x, ndc_y, aspect) {
        return false;
    }
    let play_plane = create_plane(1.0, 0.0, 0.0, -(STACK_X as f32));
    let distance = intersect_ray3d_plane(&ray, &play_plane);
    if distance < 0.0 {
        return false;
    }
    let mut point = create_vector3(0.0, 0.0, 0.0);
    get_ray3d_point_at(&mut point, &ray, distance);
    out.x = STACK_Z as f32 - point.z;
    out.y = point.y - STACK_BASE_Y as f32;
    true
}
