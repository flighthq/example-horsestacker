//! Stack-following camera state, corresponding to `scene/cameraRig.ts`.

use flight_sdk::{
    Camera3D, OrbitCameraController, OrbitCameraControllerOptions, Projection,
    create_orbit_camera_controller, create_vector3, update_orbit_camera_controller,
};

use crate::game::{
    CAMERA_EARLY_HEADROOM, CAMERA_HEIGHT_COLLAPSE_RATE, CAMERA_HEIGHT_DEADBAND,
    CAMERA_HEIGHT_FALL_RATE, CAMERA_HEIGHT_RISE_RATE, CAMERA_MAX_DISTANCE, CAMERA_MIN_DISTANCE,
    CAMERA_PILE_FILL, CAMERA_PILE_FILL_AT_HEIGHT, CAMERA_TOP_BIAS, STACK_BASE_Y, STACK_X, STACK_Z,
};
use crate::physics::{HORSE_HALF_HEIGHT, PASTURE_TOP_Y};

pub struct CameraRig {
    pub controller: OrbitCameraController,
    camera_stack_height: f32,
}

pub fn create_camera_rig() -> CameraRig {
    let controller = create_orbit_camera_controller(Some(&OrbitCameraControllerOptions {
        azimuth: Some(std::f32::consts::FRAC_PI_2),
        distance: Some(0.82),
        max_distance: Some(7.8),
        max_polar: None,
        min_distance: Some(0.68),
        min_polar: Some(0.02),
        polar: Some(0.08),
        smooth_time: Some(0.2),
        target: Some(create_vector3(STACK_X as f32, 0.1, STACK_Z as f32)),
    }));
    CameraRig {
        controller,
        camera_stack_height: 0.0,
    }
}

impl CameraRig {
    pub const fn followed_height(&self) -> f32 {
        self.camera_stack_height
    }

    pub fn is_moving(&self) -> bool {
        (self.controller.distance - self.controller.goal_distance).abs() > 0.001
            || (self.controller.polar - self.controller.goal_polar).abs() > 0.0001
            || (self.controller.azimuth - self.controller.goal_azimuth).abs() > 0.0001
    }

    pub fn reset_height(&mut self) {
        self.camera_stack_height = 0.0;
    }

    pub fn update(
        &mut self,
        camera: &mut Camera3D,
        delta_time: f32,
        measured_height: f32,
        objects_dropped: u32,
    ) -> bool {
        let height = self.follow_stack_height(measured_height, delta_time);
        let rise = (height / 1.1).clamp(0.0, 1.0);
        let herd_progress = (objects_dropped as f32 / 50.0).clamp(0.0, 1.0);
        let resting_horse_top = PASTURE_TOP_Y as f32 + HORSE_HALF_HEIGHT as f32 * 1.2;
        let tan_half_fov = match &camera.projection {
            Projection::Perspective(projection) => (projection.read().unwrap().fov_y / 2.0).tan(),
            _ => 1.0,
        };
        let visible_half_height = self.controller.distance * tan_half_fov;
        let pile_bottom_y = STACK_BASE_Y as f32 + PASTURE_TOP_Y as f32;
        let headroom = CAMERA_EARLY_HEADROOM as f32 * (1.0 - rise);
        let pile_top_y = STACK_BASE_Y as f32
            + resting_horse_top.max(height + HORSE_HALF_HEIGHT as f32 * 0.2)
            + headroom;
        let pile_span = (pile_top_y - pile_bottom_y).max(0.0001);
        let desired_target_y =
            (pile_bottom_y + pile_top_y) / 2.0 + CAMERA_TOP_BIAS as f32 * visible_half_height;
        let moved = {
            let target = self.controller.target.read().unwrap();
            (desired_target_y - target.y).abs() > 0.001
        };
        let follow = 1.0 - (-delta_time * 2.4).exp();
        {
            let mut target = self.controller.target.write().unwrap();
            target.y += (desired_target_y - target.y) * follow;
            target.x += (STACK_X as f32 - target.x) * follow;
            target.z = STACK_Z as f32;
        }
        self.controller.goal_azimuth =
            std::f32::consts::FRAC_PI_2 + rise * 0.18 + herd_progress * 0.04;
        self.controller.goal_polar = 0.06 + 0.6 * rise.min(0.45) - 0.34 * (rise - 0.45).max(0.0);
        let fill =
            CAMERA_PILE_FILL as f32 + (CAMERA_PILE_FILL_AT_HEIGHT - CAMERA_PILE_FILL) as f32 * rise;
        self.controller.goal_distance = (pile_span / (2.0 * fill) / tan_half_fov)
            .clamp(CAMERA_MIN_DISTANCE as f32, CAMERA_MAX_DISTANCE as f32);
        update_orbit_camera_controller(&mut self.controller, camera, delta_time);
        moved
    }

    fn follow_stack_height(&mut self, measured: f32, delta_time: f32) -> f32 {
        let difference = measured - self.camera_stack_height;
        if difference.abs() <= CAMERA_HEIGHT_DEADBAND as f32 {
            return self.camera_stack_height;
        }
        let fall_rate =
            CAMERA_HEIGHT_FALL_RATE as f32 + difference.abs() * CAMERA_HEIGHT_COLLAPSE_RATE as f32;
        let limit = if difference > 0.0 {
            CAMERA_HEIGHT_RISE_RATE as f32
        } else {
            fall_rate
        } * delta_time;
        self.camera_stack_height += if difference > 0.0 {
            difference.min(limit)
        } else {
            difference.max(-limit)
        };
        self.camera_stack_height
    }
}

#[cfg(test)]
mod tests {
    use super::create_camera_rig;
    use crate::scene::create_scene_graph;

    #[test]
    fn follows_height_at_the_typescript_rate_and_resets() {
        let mut scene = create_scene_graph();
        let mut rig = create_camera_rig();
        rig.update(&mut scene.camera, 0.1, 1.0, 10);
        assert!((rig.followed_height() - 0.11).abs() < 1e-6);
        assert!(rig.controller.goal_distance >= 1.7);
        rig.reset_height();
        assert_eq!(rig.followed_height(), 0.0);
    }
}
