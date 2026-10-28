//! Dust and celebration emitters, corresponding to `scene/particleEffects.ts`.

use flight_sdk::{
    EmitterShape, NodeRef, ParticleEmitter3D, ParticleEmitterConfig, ParticleEmitterConfigOptions,
    ParticleEmitterState, Ref, add_node_child, build_particle_curve, clear_particle_emitter_3d,
    create_particle_emitter_3d, create_particle_emitter_config, create_particle_emitter_state,
    emit_particle_burst_3d, step_particle_emitter_3d,
};

use crate::game::{STACK_X, STACK_Z};

const CELEBRATION_COLORS: [u32; 7] = [
    0xffd166ff, 0xef8354ff, 0x7ea16bff, 0xf7ede2ff, 0x8ecae6ff, 0xe5989bff, 0xb08fd8ff,
];
const DUST_COLOR: u32 = 0xe8d6a9cc;
// Keep the TS-authored range recognizable while compensating for the native pass presenting
// an untextured particle's scale as its full world-space side length. At the farm's scale, the raw
// 0.05..0.135 range reads as paper sheets; half-size reads as individual pieces of confetti.
const NATIVE_CELEBRATION_SCALE: f32 = 0.5;

pub struct ParticleEffects {
    dust_emitter: Ref<ParticleEmitter3D>,
    celebration_emitter: Ref<ParticleEmitter3D>,
    dust_config: ParticleEmitterConfig,
    celebration_config: ParticleEmitterConfig,
    dust_state: ParticleEmitterState,
    celebration_state: ParticleEmitterState,
    random_state: u64,
}

pub fn create_particle_effects(scene_root: &NodeRef) -> ParticleEffects {
    let dust_emitter = create_particle_emitter_3d();
    {
        let mut emitter = dust_emitter.write().unwrap();
        emitter.name = Some("horse-impact-dust".to_owned());
        emitter.blend_mode = Some("normal".to_owned());
        emitter.data.world_space = true;
    }
    let dust_node: NodeRef = dust_emitter.clone();
    add_node_child(scene_root, &dust_node);

    let celebration_emitter = create_particle_emitter_3d();
    {
        let mut emitter = celebration_emitter.write().unwrap();
        emitter.name = Some("horse-confetti".to_owned());
        emitter.blend_mode = Some("normal".to_owned());
        emitter.data.world_space = true;
    }
    let celebration_node: NodeRef = celebration_emitter.clone();
    add_node_child(scene_root, &celebration_node);

    ParticleEffects {
        dust_emitter,
        celebration_emitter,
        dust_config: create_dust_config(),
        celebration_config: create_celebration_config(),
        dust_state: create_particle_emitter_state(),
        celebration_state: create_particle_emitter_state(),
        random_state: 0x48f1_23ab_8d71_55c9,
    }
}

impl ParticleEffects {
    pub fn burst_dust(&mut self, physics_x: f32, world_y: f32) {
        emit_particle_burst_3d(
            &self.dust_emitter,
            &mut self.dust_state,
            &self.dust_config,
            8.0,
            STACK_X as f32,
            world_y,
            STACK_Z as f32 - physics_x,
            Some(DUST_COLOR),
        );
    }

    pub fn burst_celebration(&mut self, world_y: f32) {
        let denominator = (CELEBRATION_COLORS.len() - 1) as f32;
        for (index, color) in CELEBRATION_COLORS.into_iter().enumerate() {
            let horizontal_offset = -1.8 + index as f32 / denominator * 3.6;
            let jitter = (self.next_random() - 0.5) * 0.16;
            emit_particle_burst_3d(
                &self.celebration_emitter,
                &mut self.celebration_state,
                &self.celebration_config,
                92.0,
                STACK_X as f32 + jitter,
                world_y,
                STACK_Z as f32 - horizontal_offset,
                Some(color),
            );
        }
    }

    pub fn reset(&mut self) {
        clear_particle_emitter_3d(&self.dust_emitter);
        clear_particle_emitter_3d(&self.celebration_emitter);
        self.dust_state = create_particle_emitter_state();
        self.celebration_state = create_particle_emitter_state();
    }

    pub fn step(&mut self, delta_time: f32) -> bool {
        let dust_is_moving = self.dust_emitter.read().unwrap().data.particle_count > 0;
        let celebration_is_moving =
            self.celebration_emitter.read().unwrap().data.particle_count > 0;
        if dust_is_moving {
            step_particle_emitter_3d(
                &self.dust_emitter,
                &mut self.dust_state,
                &self.dust_config,
                delta_time,
                None,
                None,
                None,
            );
        }
        if celebration_is_moving {
            step_particle_emitter_3d(
                &self.celebration_emitter,
                &mut self.celebration_state,
                &self.celebration_config,
                delta_time,
                None,
                None,
                None,
            );
        }
        dust_is_moving || celebration_is_moving
    }

    fn next_random(&mut self) -> f32 {
        self.random_state ^= self.random_state << 13;
        self.random_state ^= self.random_state >> 7;
        self.random_state ^= self.random_state << 17;
        (self.random_state as u32) as f32 / u32::MAX as f32
    }
}

fn create_celebration_config() -> ParticleEmitterConfig {
    let mut alpha_curve = Vec::new();
    build_particle_curve(
        &mut alpha_curve,
        &|t| {
            if t < 0.74 {
                1.0
            } else {
                1.0 - (t - 0.74) / 0.26
            }
        },
        32,
    );
    create_particle_emitter_config(Some(&ParticleEmitterConfigOptions {
        alpha_curve: Some(Some(alpha_curve)),
        alpha_end: Some(1.0),
        alpha_start: Some(1.0),
        color_end_b: Some(1.0),
        color_end_g: Some(1.0),
        color_end_r: Some(1.0),
        color_start_b: Some(1.0),
        color_start_g: Some(1.0),
        color_start_r: Some(1.0),
        direction_x: Some(0.0),
        direction_y: Some(1.0),
        direction_z: Some(0.0),
        duration: Some(0.0),
        emitter_cone_angle: Some(2.5),
        emitter_radius: Some(0.22),
        emitter_shape: Some(EmitterShape::Cone3D),
        gravity_y: Some(-1.15),
        lifetime_max: Some(4.4),
        lifetime_min: Some(2.4),
        looping: Some(false),
        max_particles: Some(760.0),
        rotation_speed_max: Some(17.0),
        rotation_speed_min: Some(-17.0),
        scale_end: Some(1.0),
        scale_max: Some(0.135 * NATIVE_CELEBRATION_SCALE),
        scale_min: Some(0.05 * NATIVE_CELEBRATION_SCALE),
        spawn_rate: Some(0.0),
        speed_max: Some(3.4),
        speed_min: Some(1.5),
        world_space: Some(true),
        ..ParticleEmitterConfigOptions::default()
    }))
}

fn create_dust_config() -> ParticleEmitterConfig {
    create_particle_emitter_config(Some(&ParticleEmitterConfigOptions {
        alpha_end: Some(0.0),
        alpha_start: Some(0.42),
        color_end_b: Some(0.58),
        color_end_g: Some(0.69),
        color_end_r: Some(0.76),
        color_start_b: Some(0.72),
        color_start_g: Some(0.8),
        color_start_r: Some(0.86),
        direction_x: Some(0.0),
        direction_y: Some(1.0),
        direction_z: Some(0.0),
        duration: Some(0.0),
        emitter_cone_angle: Some(1.35),
        emitter_radius: Some(0.012),
        emitter_shape: Some(EmitterShape::Cone3D),
        gravity_y: Some(-0.5),
        lifetime_max: Some(0.42),
        lifetime_min: Some(0.18),
        looping: Some(false),
        max_particles: Some(96.0),
        rotation_speed_max: Some(2.0),
        rotation_speed_min: Some(-2.0),
        scale_end: Some(0.002),
        scale_max: Some(0.036),
        scale_min: Some(0.008),
        spawn_rate: Some(0.0),
        speed_max: Some(0.35),
        speed_min: Some(0.08),
        world_space: Some(true),
        ..ParticleEmitterConfigOptions::default()
    }))
}

#[cfg(test)]
mod tests {
    use super::{NATIVE_CELEBRATION_SCALE, create_celebration_config, create_dust_config};

    #[test]
    fn scales_native_confetti_without_rescaling_impact_dust() {
        let celebration = create_celebration_config();
        assert_eq!(celebration.scale_min, 0.05 * NATIVE_CELEBRATION_SCALE);
        assert_eq!(celebration.scale_max, 0.135 * NATIVE_CELEBRATION_SCALE);

        let dust = create_dust_config();
        assert_eq!(dust.scale_min, 0.008);
        assert_eq!(dust.scale_max, 0.036);
    }
}
