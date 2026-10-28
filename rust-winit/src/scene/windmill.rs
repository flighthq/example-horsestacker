//! The farm's turning windmill sails, corresponding to `scene/windmill.ts`.

use flight_sdk::{
    EulerOrder, Mesh, Node3D, NodeRef, Ref, Scene3D, downcast_node_ref, find_node_by_name,
    invalidate_node_local_transform, refresh_mesh_geometry_bounds, set_quaternion_from_euler,
};

use crate::game::WINDMILL_RADIANS_PER_SECOND;
use crate::reduced_motion::prefers_reduced_motion;

pub struct Windmill {
    sails: Ref<Mesh>,
    hub_y: f32,
    hub_z: f32,
    angle: f32,
}

pub fn create_windmill(farm: &Scene3D) -> Result<Windmill, String> {
    let farm_root: NodeRef = farm.root.clone();
    let sails_node = find_node_by_name(&farm_root, "Object_6")
        .ok_or_else(|| "Windmill sail mesh Object_6 was not imported".to_owned())?;
    let sails = downcast_node_ref::<Mesh>(&sails_node)
        .ok_or_else(|| "Windmill sail mesh Object_6 was not imported".to_owned())?;
    let material_name = sails
        .read()
        .unwrap()
        .materials
        .read()
        .unwrap()
        .first()
        .and_then(Option::as_ref)
        .and_then(|material| material.read().unwrap().material_fields().name.clone());
    if material_name.as_deref() != Some("Windmill2") {
        return Err(format!(
            "Windmill sail mesh Object_6 uses {}, expected Windmill2",
            material_name.as_deref().unwrap_or("no material")
        ));
    }
    {
        let geometry = sails.read().unwrap().geometry.clone();
        refresh_mesh_geometry_bounds(&mut geometry.write().unwrap());
    }
    let bounds = sails
        .read()
        .unwrap()
        .geometry
        .read()
        .unwrap()
        .bounds
        .clone()
        .ok_or_else(|| "Windmill sail mesh has no bounds".to_owned())?;
    let mut windmill = Windmill {
        sails,
        hub_y: (bounds.min.y + bounds.max.y) * 0.5,
        hub_z: (bounds.min.z + bounds.max.z) * 0.5,
        angle: 0.0,
    };
    windmill.turn(0.0);
    Ok(windmill)
}

impl Windmill {
    /// Advances the sails and returns true when they moved.
    pub fn update(&mut self, delta_time: f32) -> bool {
        self.turn(delta_time)
    }

    fn turn(&mut self, delta_time: f32) -> bool {
        if prefers_reduced_motion() {
            return false;
        }
        self.angle =
            (self.angle + WINDMILL_RADIANS_PER_SECOND as f32 * delta_time) % std::f32::consts::TAU;
        let sin = self.angle.sin();
        let cos = self.angle.cos();
        {
            let mut sails = self.sails.write().unwrap();
            let fields = sails.node3d_fields_mut();
            set_quaternion_from_euler(&mut fields.rotation, self.angle, 0.0, 0.0, EulerOrder::XYZ);
            fields.position.y = self.hub_y - (self.hub_y * cos - self.hub_z * sin);
            fields.position.z = self.hub_z - (self.hub_y * sin + self.hub_z * cos);
        }
        let sails_node: NodeRef = self.sails.clone();
        invalidate_node_local_transform(&sails_node);
        true
    }
}
