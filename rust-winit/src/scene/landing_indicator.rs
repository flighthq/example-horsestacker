//! The gold held-piece marker, corresponding to `scene/landingIndicator.ts`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use flight_sdk::{
    EulerOrder, Material3D, MaterialAlphaMode, Node3DFields, NodeRef, PointLight, Ref,
    Scene3DMaterialOverride, StandardPbrMaterial, add_node_child, as_node3d_ref, create_mesh,
    create_node3d, create_ring_mesh_geometry, create_standard_pbr_material,
    invalidate_node_local_transform, publish_node3d, remove_node_children, set_node3d_alpha,
    set_quaternion_from_euler,
};

use crate::game::{
    INDICATOR_DAMPING, INDICATOR_MAX_ANGLE, INDICATOR_MAX_SPIN, INDICATOR_SPRING, STACK_BASE_Y,
    STACK_X, STACK_Z,
};
use crate::physics::{
    PASTURE_MAX_X, PASTURE_MIN_X, StackObjectKind, get_stack_object_vertical_extent,
};
use crate::reduced_motion::prefers_reduced_motion;

use super::StackObjectVisuals;

const PREVIEW_TINT_MIX: f32 = 0.42;
const PREVIEW_GLOW_MIX: f32 = 0.22;

pub struct LandingIndicator {
    preview_layer: NodeRef,
    indicator_light: Ref<PointLight>,
    ghost_material: Ref<dyn Material3D>,
    halo_material: Ref<dyn Material3D>,
    preview_materials: Arc<Mutex<HashMap<usize, Ref<dyn Material3D>>>>,
    ghost: Option<NodeRef>,
    radiance: Option<NodeRef>,
    angle: f64,
    angular_velocity: f64,
    updated_at: f64,
    preview_top_y: f64,
}

pub fn create_landing_indicator(
    preview_layer: NodeRef,
    indicator_light: Ref<PointLight>,
) -> LandingIndicator {
    let mut ghost_material = create_standard_pbr_material();
    ghost_material.alpha_mode = MaterialAlphaMode::Opaque;
    ghost_material.base_color = 0xe2b83fff;
    ghost_material.double_sided = true;
    ghost_material.emissive = 0x8a5a0bff;
    ghost_material.emissive_strength = 0.7;
    ghost_material.metallic = 0.18;
    ghost_material.roughness = 0.38;
    let ghost_material: Ref<dyn Material3D> = Arc::new(RwLock::new(ghost_material));

    let mut halo_material = create_standard_pbr_material();
    halo_material.alpha_mode = MaterialAlphaMode::Blend;
    halo_material.base_color = 0xf5d36aff;
    halo_material.double_sided = true;
    halo_material.emissive = 0xd49a22ff;
    halo_material.emissive_strength = 1.15;
    halo_material.metallic = 0.1;
    halo_material.roughness = 0.42;
    let halo_material: Ref<dyn Material3D> = Arc::new(RwLock::new(halo_material));

    LandingIndicator {
        preview_layer,
        indicator_light,
        ghost_material,
        halo_material,
        preview_materials: Arc::new(Mutex::new(HashMap::new())),
        ghost: None,
        radiance: None,
        angle: 0.0,
        angular_velocity: 0.0,
        updated_at: 0.0,
        preview_top_y: 0.0,
    }
}

impl LandingIndicator {
    pub const fn angle(&self) -> f64 {
        self.angle
    }

    pub const fn preview_top_y(&self) -> f64 {
        self.preview_top_y
    }

    pub fn begin_round(&mut self) {
        let ghost = create_group(Some("landing-preview"));
        as_node3d_ref(&ghost)
            .expect("landing preview is Node3D")
            .write()
            .unwrap()
            .node3d_fields_mut()
            .alpha = 1.0;
        add_node_child(&self.preview_layer, &ghost);
        let radiance = self.create_radiance();
        add_node_child(&self.preview_layer, &radiance);
        self.indicator_light.write().unwrap().intensity = 0.0;
        self.ghost = Some(ghost);
        self.radiance = Some(radiance);
    }

    pub fn set_kind(
        &mut self,
        visuals: &StackObjectVisuals,
        kind: StackObjectKind,
        variant_index: usize,
    ) -> Result<(), String> {
        let Some(ghost) = &self.ghost else {
            return Ok(());
        };
        remove_node_children(ghost, None, None);
        let mapper = self.preview_material_mapper();
        let preview = visuals.create(kind, variant_index, Some(mapper), 1.0)?;
        add_node_child(ghost, &preview);
        {
            let node3d = as_node3d_ref(ghost).expect("landing preview is Node3D");
            let mut node3d = node3d.write().unwrap();
            let fields = node3d.node3d_fields_mut();
            fields.name = Some(format!("{}-landing-preview", stack_object_key(kind)));
            fields.enabled = true;
        }
        if let Some(radiance) = &self.radiance {
            set_enabled(radiance, true);
        }
        Ok(())
    }

    pub fn reset_teeter(&mut self, now: f64) {
        self.angle = 0.0;
        self.angular_velocity = 0.0;
        self.updated_at = now;
    }

    pub fn nudge(&mut self, pointer_velocity: f64) {
        self.angular_velocity = (self.angular_velocity - (pointer_velocity * 0.32).clamp(-4.2, 4.2))
            .clamp(-INDICATOR_MAX_SPIN, INDICATOR_MAX_SPIN);
    }

    pub fn step_teeter(&mut self, now: f64) {
        let delta_time = ((now - self.updated_at) / 1_000.0).clamp(0.0, 0.05);
        self.updated_at = now;
        if delta_time == 0.0 {
            return;
        }
        let acceleration =
            -self.angle * INDICATOR_SPRING - self.angular_velocity * INDICATOR_DAMPING;
        self.angular_velocity = (self.angular_velocity + acceleration * delta_time)
            .clamp(-INDICATOR_MAX_SPIN, INDICATOR_MAX_SPIN);
        self.angle = (self.angle + self.angular_velocity * delta_time)
            .clamp(-INDICATOR_MAX_ANGLE, INDICATOR_MAX_ANGLE);
    }

    pub fn update(
        &mut self,
        visuals: &StackObjectVisuals,
        kind: StackObjectKind,
        x: f64,
        y: f64,
        piece_angle: f64,
        now: f64,
    ) {
        let Some(ghost) = &self.ghost else {
            return;
        };
        let visible = (PASTURE_MIN_X..=PASTURE_MAX_X).contains(&x);
        set_enabled(ghost, visible);
        if !visible {
            if let Some(radiance) = &self.radiance {
                set_enabled(radiance, false);
            }
            self.indicator_light.write().unwrap().intensity = 0.0;
            return;
        }
        self.preview_top_y = y + get_stack_object_vertical_extent(kind, piece_angle);
        visuals.set_transform(ghost, kind, x as f32, y as f32, piece_angle as f32);
        self.update_radiance(x, y, now);
    }

    pub fn hide(&mut self) {
        if let Some(ghost) = &self.ghost {
            set_enabled(ghost, false);
        }
        if let Some(radiance) = &self.radiance {
            set_enabled(radiance, false);
        }
        self.indicator_light.write().unwrap().intensity = 0.0;
    }

    fn create_radiance(&self) -> NodeRef {
        let root = create_group(Some("landing-radiance"));
        let geometry = Arc::new(RwLock::new(create_ring_mesh_geometry(
            Some(0.105),
            Some(0.132),
            Some(28),
        )));
        let materials = Arc::new(RwLock::new(vec![Some(self.halo_material.clone())]));
        let halo = create_mesh(geometry, materials);
        {
            let mut halo = halo.write().unwrap();
            halo.alpha = 0.24;
            halo.position.x = 0.012;
            set_quaternion_from_euler(
                &mut halo.rotation,
                0.0,
                0.0,
                std::f32::consts::FRAC_PI_2,
                EulerOrder::XYZ,
            );
        }
        let halo_node: NodeRef = halo;
        invalidate_node_local_transform(&halo_node);
        add_node_child(&root, &halo_node);
        root
    }

    fn preview_material_mapper(&self) -> Scene3DMaterialOverride {
        let ghost_material = self.ghost_material.clone();
        let preview_materials = self.preview_materials.clone();
        Arc::new(move |source| {
            let Some(source) = source else {
                return Some(ghost_material.clone());
            };
            let key = Arc::as_ptr(&source) as *const () as usize;
            if let Some(cached) = preview_materials.lock().unwrap().get(&key) {
                return Some(cached.clone());
            }
            let blended: Ref<dyn Material3D> = {
                let source = source.read().unwrap();
                let Some(pbr) = source.as_any().downcast_ref::<StandardPbrMaterial>() else {
                    preview_materials
                        .lock()
                        .unwrap()
                        .insert(key, ghost_material.clone());
                    return Some(ghost_material.clone());
                };
                let mut blended = pbr.clone();
                let ghost = ghost_material.read().unwrap();
                let ghost = ghost
                    .as_any()
                    .downcast_ref::<StandardPbrMaterial>()
                    .expect("preview marker material is standard PBR");
                blended.base_color = mix_rgba(pbr.base_color, ghost.base_color, PREVIEW_TINT_MIX);
                blended.emissive = mix_rgba(pbr.emissive, ghost.emissive, PREVIEW_GLOW_MIX);
                blended.emissive_strength = pbr.emissive_strength
                    + (ghost.emissive_strength - pbr.emissive_strength) * PREVIEW_GLOW_MIX;
                blended.metallic =
                    pbr.metallic + (ghost.metallic - pbr.metallic) * PREVIEW_TINT_MIX;
                blended.roughness =
                    pbr.roughness + (ghost.roughness - pbr.roughness) * PREVIEW_TINT_MIX;
                blended.double_sided = true;
                Arc::new(RwLock::new(blended))
            };
            preview_materials
                .lock()
                .unwrap()
                .insert(key, blended.clone());
            Some(blended)
        })
    }

    fn update_radiance(&mut self, x: f64, physics_y: f64, now: f64) {
        let Some(radiance) = &self.radiance else {
            return;
        };
        let pulse = if prefers_reduced_motion() {
            1.0
        } else {
            1.0 + (now * 0.006).sin() as f32 * 0.025
        };
        set_enabled(radiance, true);
        set_node3d_alpha(radiance, 0.48 + (now * 0.008).sin() as f32 * 0.055);
        {
            let radiance = as_node3d_ref(radiance).expect("radiance is Node3D");
            let mut radiance = radiance.write().unwrap();
            let fields = radiance.node3d_fields_mut();
            fields.position.x = STACK_X as f32 + 0.006;
            fields.position.y = (STACK_BASE_Y + physics_y) as f32;
            fields.position.z = (STACK_Z - x) as f32;
            fields.scale.x = pulse;
            fields.scale.y = pulse;
            fields.scale.z = pulse;
        }
        invalidate_node_local_transform(radiance);
        let mut light = self.indicator_light.write().unwrap();
        light.position.x = STACK_X as f32 + 0.1;
        light.position.y = (STACK_BASE_Y + physics_y) as f32 + 0.09;
        light.position.z = (STACK_Z - x) as f32;
        light.intensity = 0.34 + (now * 0.008).sin() as f32 * 0.07;
    }
}

fn create_group(name: Option<&str>) -> NodeRef {
    let mut group = create_node3d::<Node3DFields>(None);
    group.name = name.map(str::to_owned);
    publish_node3d(group)
}

fn set_enabled(node: &NodeRef, enabled: bool) {
    as_node3d_ref(node)
        .expect("landing indicator node is Node3D")
        .write()
        .unwrap()
        .node3d_fields_mut()
        .enabled = enabled;
}

fn mix_channel(from: u32, to: u32, shift: u32, amount: f32) -> u32 {
    let a = (from >> shift) & 0xff;
    let b = (to >> shift) & 0xff;
    ((a as f32 + (b as f32 - a as f32) * amount).round() as u32) << shift
}

fn mix_rgba(from: u32, to: u32, amount: f32) -> u32 {
    mix_channel(from, to, 24, amount)
        | mix_channel(from, to, 16, amount)
        | mix_channel(from, to, 8, amount)
        | (from & 0xff)
}

const fn stack_object_key(kind: StackObjectKind) -> &'static str {
    match kind {
        StackObjectKind::Horse => "horse",
        StackObjectKind::Hay => "hay",
        StackObjectKind::Cow => "cow",
        StackObjectKind::Chickens => "chickens",
    }
}
