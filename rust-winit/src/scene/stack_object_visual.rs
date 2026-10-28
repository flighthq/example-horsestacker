//! Stack-object scene instances, corresponding to `scene/stackObjectVisual.ts`.

use std::collections::HashMap;

use flight_sdk::node::contract::get_node_local_matrix4;
use flight_sdk::{
    EulerOrder, InstancedMesh, Matrix4, MeshGeometry, MeshMaterials, Node3DFields, NodeRef,
    Quaternion, Ref, Scene3D, Scene3DMaterialOverride, Vector3, add_node_child, as_node3d_ref,
    clone_node3d_subtree, compose_matrix4, create_instanced_mesh, create_matrix4, create_node3d,
    create_quaternion, create_vector3, get_node_children, invalidate_instanced_mesh,
    invalidate_node_local_transform, multiply_matrix4, publish_node3d,
    set_instanced_mesh_instance_count, set_instanced_mesh_instance_matrix,
    set_quaternion_from_euler,
};

use crate::data::{FARM_PROP_VARIANTS, FarmPropKind};
use crate::game::{HORSE_SCALE, HORSE_VISUAL_CENTER_Y, STACK_BASE_Y, STACK_X, STACK_Z};
use crate::physics::{StackObjectKind, get_collider_centroid_offset_y, get_stack_object_profile};

use super::FarmPropTemplates;

pub type MaterialMapper = Scene3DMaterialOverride;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct BatchKey {
    kind: StackObjectKind,
    variant_index: usize,
}

#[derive(Clone, Debug)]
pub struct StackVisualHandle {
    pub kind: StackObjectKind,
    pub variant_index: usize,
    batch_key: BatchKey,
    last_frame: i64,
    frame_index: i32,
}

struct MeshPart {
    instanced_mesh: Ref<InstancedMesh>,
    part_matrix: Matrix4,
}

struct Batch {
    parts: Vec<MeshPart>,
    frame_count: i32,
}

struct CollectedMeshPart {
    geometry: Ref<MeshGeometry>,
    materials: MeshMaterials,
    local_matrix: Matrix4,
}

const INITIAL_CAPACITY: usize = 32;

pub struct StackObjectVisuals {
    farm_prop_templates: FarmPropTemplates,
    horse_template: Option<Scene3D>,
    batches: HashMap<BatchKey, Batch>,
    all_instanced_nodes: Vec<NodeRef>,
    frame_number: i64,
    tmp_position: Vector3,
    tmp_rotation: Quaternion,
    tmp_scale: Vector3,
    tmp_world_matrix: Matrix4,
    tmp_instance_matrix: Matrix4,
}

pub fn create_stack_object_visuals() -> StackObjectVisuals {
    StackObjectVisuals {
        farm_prop_templates: FarmPropTemplates::default(),
        horse_template: None,
        batches: HashMap::new(),
        all_instanced_nodes: Vec::new(),
        frame_number: 0,
        tmp_position: create_vector3(0.0, 0.0, 0.0),
        tmp_rotation: create_quaternion(0.0, 0.0, 0.0, 1.0),
        tmp_scale: create_vector3(1.0, 1.0, 1.0),
        tmp_world_matrix: create_matrix4(),
        tmp_instance_matrix: create_matrix4(),
    }
}

impl StackObjectVisuals {
    pub fn set_templates(&mut self, farm_props: FarmPropTemplates, horse: Scene3D) {
        self.farm_prop_templates = farm_props;
        self.horse_template = Some(horse);
        self.build_farm_prop_batches();
        self.build_horse_batch();
    }

    pub const fn is_ready(&self) -> bool {
        self.horse_template.is_some()
    }

    pub fn create(
        &self,
        kind: StackObjectKind,
        variant_index: usize,
        material_mapper: Option<MaterialMapper>,
        alpha: f32,
    ) -> Result<NodeRef, String> {
        let pivot = create_group(None);
        as_node3d_ref(&pivot)
            .expect("stack visual pivot is Node3D")
            .write()
            .unwrap()
            .node3d_fields_mut()
            .alpha = alpha;

        if kind != StackObjectKind::Horse {
            let farm_kind = farm_prop_kind(kind).expect("non-horse kind is a farm prop");
            let templates = self.farm_prop_templates.get(farm_kind);
            let template = templates
                .get(variant_index)
                .or_else(|| templates.first())
                .ok_or_else(|| format!("{} is not loaded", get_stack_object_profile(kind).label))?;
            add_node_child(&pivot, &clone_node3d_subtree(template, material_mapper));
            return Ok(pivot);
        }

        let horse = self
            .horse_template
            .as_ref()
            .ok_or_else(|| "Horse model is not loaded".to_owned())?;
        let model_transform = create_horse_model_transform();
        let horse_root: NodeRef = horse.root.clone();
        add_node_child(
            &model_transform,
            &clone_node3d_subtree(&horse_root, material_mapper),
        );
        add_node_child(&pivot, &model_transform);
        Ok(pivot)
    }

    pub fn add_piece(
        &self,
        kind: StackObjectKind,
        variant_index: usize,
    ) -> Result<StackVisualHandle, String> {
        let batch_key = BatchKey {
            kind,
            variant_index,
        };
        if !self.batches.contains_key(&batch_key) {
            return Err(format!(
                "No batch for {}-{variant_index}",
                stack_object_key(kind)
            ));
        }
        Ok(StackVisualHandle {
            kind,
            variant_index,
            batch_key,
            last_frame: -1,
            frame_index: -1,
        })
    }

    pub fn set_transform(
        &self,
        node: &NodeRef,
        kind: StackObjectKind,
        x: f32,
        physics_y: f32,
        angle: f32,
    ) {
        let node3d = as_node3d_ref(node).expect("stack visual is Node3D");
        {
            let mut node3d = node3d.write().unwrap();
            let fields = node3d.node3d_fields_mut();
            fields.position.x = STACK_X as f32;
            fields.position.y = STACK_BASE_Y as f32 + physics_y + visual_offset_y(kind);
            fields.position.z = STACK_Z as f32 - x;
            set_quaternion_from_euler(
                &mut fields.rotation,
                if kind == StackObjectKind::Chickens {
                    0.0
                } else {
                    angle
                },
                0.0,
                0.0,
                EulerOrder::XYZ,
            );
        }
        invalidate_node_local_transform(node);
    }

    pub fn set_piece_transform(
        &mut self,
        handle: &mut StackVisualHandle,
        kind: StackObjectKind,
        x: f32,
        physics_y: f32,
        angle: f32,
    ) {
        let Some(batch) = self.batches.get_mut(&handle.batch_key) else {
            return;
        };
        let index = if handle.last_frame == self.frame_number {
            handle.frame_index
        } else {
            let index = batch.frame_count;
            batch.frame_count += 1;
            handle.frame_index = index;
            handle.last_frame = self.frame_number;
            index
        };

        self.tmp_position.x = 0.0;
        self.tmp_position.y = STACK_BASE_Y as f32 + physics_y + visual_offset_y(kind);
        self.tmp_position.z = -x;
        set_quaternion_from_euler(
            &mut self.tmp_rotation,
            if kind == StackObjectKind::Chickens {
                0.0
            } else {
                angle
            },
            0.0,
            0.0,
            EulerOrder::XYZ,
        );
        self.tmp_scale.x = 1.0;
        self.tmp_scale.y = 1.0;
        self.tmp_scale.z = 1.0;
        compose_matrix4(
            &mut self.tmp_world_matrix,
            &self.tmp_position,
            &self.tmp_rotation,
            &self.tmp_scale,
        );

        for part in &batch.parts {
            multiply_matrix4(
                &mut self.tmp_instance_matrix,
                &self.tmp_world_matrix,
                &part.part_matrix,
            );
            if index >= part.instanced_mesh.read().unwrap().instance_count {
                set_instanced_mesh_instance_count(&part.instanced_mesh, index + 1);
            }
            set_instanced_mesh_instance_matrix(
                &part.instanced_mesh,
                index,
                &self.tmp_instance_matrix,
            );
        }
    }

    pub fn flush(&mut self) {
        for batch in self.batches.values_mut() {
            for part in &batch.parts {
                set_instanced_mesh_instance_count(&part.instanced_mesh, batch.frame_count);
                invalidate_instanced_mesh(&part.instanced_mesh);
            }
            batch.frame_count = 0;
        }
        self.frame_number += 1;
    }

    pub fn clear_pieces(&mut self) {
        for batch in self.batches.values_mut() {
            batch.frame_count = 0;
            for part in &batch.parts {
                set_instanced_mesh_instance_count(&part.instanced_mesh, 0);
                invalidate_instanced_mesh(&part.instanced_mesh);
            }
        }
        self.frame_number += 1;
    }

    pub fn instanced_nodes(&self) -> &[NodeRef] {
        &self.all_instanced_nodes
    }

    pub fn label(&self, kind: StackObjectKind, variant_index: usize) -> &'static str {
        if kind == StackObjectKind::Horse {
            return get_stack_object_profile(StackObjectKind::Horse).label;
        }
        let farm_kind = farm_prop_kind(kind).expect("non-horse kind is a farm prop");
        FARM_PROP_VARIANTS
            .get(farm_kind)
            .get(variant_index)
            .map(|variant| variant.label)
            .unwrap_or_else(|| get_stack_object_profile(kind).label)
    }

    fn build_batch(&mut self, template_root: &NodeRef, key: BatchKey) {
        let mesh_parts = collect_mesh_parts(template_root);
        let mut parts = Vec::with_capacity(mesh_parts.len());
        for part in mesh_parts {
            let instanced_mesh =
                create_instanced_mesh(part.geometry, part.materials, Some(INITIAL_CAPACITY));
            {
                let mut instanced_mesh_fields = instanced_mesh.write().unwrap();
                instanced_mesh_fields.position.x = STACK_X as f32;
                instanced_mesh_fields.position.z = STACK_Z as f32;
            }
            let instanced_node: NodeRef = instanced_mesh.clone();
            invalidate_node_local_transform(&instanced_node);
            set_instanced_mesh_instance_count(&instanced_mesh, 0);
            self.all_instanced_nodes.push(instanced_node);
            parts.push(MeshPart {
                instanced_mesh,
                part_matrix: part.local_matrix,
            });
        }
        self.batches.insert(
            key,
            Batch {
                parts,
                frame_count: 0,
            },
        );
    }

    fn build_horse_batch(&mut self) {
        let Some(horse) = &self.horse_template else {
            return;
        };
        let model_transform = create_horse_model_transform();
        let horse_root: NodeRef = horse.root.clone();
        add_node_child(&model_transform, &clone_node3d_subtree(&horse_root, None));
        self.build_batch(
            &model_transform,
            BatchKey {
                kind: StackObjectKind::Horse,
                variant_index: 0,
            },
        );
    }

    fn build_farm_prop_batches(&mut self) {
        let templates: Vec<(BatchKey, NodeRef)> = [
            (FarmPropKind::Hay, StackObjectKind::Hay),
            (FarmPropKind::Cow, StackObjectKind::Cow),
            (FarmPropKind::Chickens, StackObjectKind::Chickens),
        ]
        .into_iter()
        .flat_map(|(farm_kind, stack_kind)| {
            self.farm_prop_templates
                .get(farm_kind)
                .iter()
                .cloned()
                .enumerate()
                .map(move |(variant_index, template)| {
                    (
                        BatchKey {
                            kind: stack_kind,
                            variant_index,
                        },
                        template,
                    )
                })
        })
        .collect();
        for (key, template) in templates {
            self.build_batch(&template, key);
        }
    }
}

fn collect_mesh_parts(root: &NodeRef) -> Vec<CollectedMeshPart> {
    fn walk(node: &NodeRef, parent_matrix: &Matrix4, parts: &mut Vec<CollectedMeshPart>) {
        let local_matrix = get_node_local_matrix4(node);
        let mut world_matrix = create_matrix4();
        multiply_matrix4(&mut world_matrix, parent_matrix, &local_matrix);

        if let Some(node3d) = as_node3d_ref(node) {
            let node3d = node3d.read().unwrap();
            if let Some(mesh) = node3d.as_mesh() {
                parts.push(CollectedMeshPart {
                    geometry: mesh.geometry.clone(),
                    materials: mesh.materials.clone(),
                    local_matrix: world_matrix.clone(),
                });
            }
        }

        let mut children = Vec::new();
        get_node_children(&mut children, node);
        for child in children {
            walk(&child, &world_matrix, parts);
        }
    }

    let mut parts = Vec::new();
    walk(root, &create_matrix4(), &mut parts);
    parts
}

fn create_horse_model_transform() -> NodeRef {
    let model_transform = create_group(None);
    {
        let node3d = as_node3d_ref(&model_transform).expect("horse model transform is Node3D");
        let mut model_transform_fields = node3d.write().unwrap();
        let fields = model_transform_fields.node3d_fields_mut();
        fields.scale.x = HORSE_SCALE as f32;
        fields.scale.y = HORSE_SCALE as f32;
        fields.scale.z = HORSE_SCALE as f32;
        fields.position.y = -HORSE_VISUAL_CENTER_Y as f32;
        set_quaternion_from_euler(&mut fields.rotation, 0.0, 0.0, 0.0, EulerOrder::XYZ);
    }
    invalidate_node_local_transform(&model_transform);
    model_transform
}

fn create_group(name: Option<&str>) -> NodeRef {
    let mut group = create_node3d::<Node3DFields>(None);
    group.name = name.map(str::to_owned);
    publish_node3d(group)
}

const fn farm_prop_kind(kind: StackObjectKind) -> Option<FarmPropKind> {
    match kind {
        StackObjectKind::Horse => None,
        StackObjectKind::Hay => Some(FarmPropKind::Hay),
        StackObjectKind::Cow => Some(FarmPropKind::Cow),
        StackObjectKind::Chickens => Some(FarmPropKind::Chickens),
    }
}

fn visual_offset_y(kind: StackObjectKind) -> f32 {
    get_collider_centroid_offset_y(kind).unwrap_or(0.0) as f32
}

const fn stack_object_key(kind: StackObjectKind) -> &'static str {
    match kind {
        StackObjectKind::Horse => "horse",
        StackObjectKind::Hay => "hay",
        StackObjectKind::Cow => "cow",
        StackObjectKind::Chickens => "chickens",
    }
}

#[cfg(test)]
mod tests {
    use super::create_stack_object_visuals;
    use crate::assets::asset_path;
    use crate::game::{STACK_BASE_Y, STACK_X, STACK_Z};
    use crate::physics::{StackObjectKind, get_collider_centroid_offset_y};
    use crate::scene::{extract_farm_prop_templates, load_gltf_scene};
    use flight_sdk::as_node3d_ref;

    #[test]
    fn builds_and_updates_the_typescript_instanced_batches() {
        let farm = load_gltf_scene(asset_path("models/farm")).unwrap();
        let horse = load_gltf_scene(asset_path("models/horse")).unwrap();
        let templates = extract_farm_prop_templates(&farm).unwrap();
        let mut visuals = create_stack_object_visuals();
        visuals.set_templates(templates, horse);

        assert!(visuals.is_ready());
        assert!(!visuals.instanced_nodes().is_empty());
        assert_eq!(visuals.label(StackObjectKind::Chickens, 1), "Brown hen");

        let mut handle = visuals.add_piece(StackObjectKind::Horse, 0).unwrap();
        visuals.set_piece_transform(&mut handle, StackObjectKind::Horse, 0.2, 0.3, 0.1);
        visuals.flush();
        let visible_parts = visuals
            .instanced_nodes()
            .iter()
            .filter(|node| {
                let node3d = as_node3d_ref(node).unwrap();
                let node3d = node3d.read().unwrap();
                node3d
                    .as_instanced_mesh()
                    .is_some_and(|mesh| mesh.instance_count == 1)
            })
            .count();
        assert!(visible_parts > 0);

        let preview = visuals.create(StackObjectKind::Hay, 0, None, 1.0).unwrap();
        visuals.set_transform(&preview, StackObjectKind::Hay, 0.25, 0.4, 0.2);
        let preview = as_node3d_ref(&preview).unwrap();
        let preview = preview.read().unwrap();
        assert_eq!(preview.node3d_fields().position.x, STACK_X as f32);
        assert_eq!(preview.node3d_fields().position.z, STACK_Z as f32 - 0.25);
        let expected_y = STACK_BASE_Y as f32
            + 0.4
            + get_collider_centroid_offset_y(StackObjectKind::Hay).unwrap_or(0.0) as f32;
        assert!((preview.node3d_fields().position.y - expected_y).abs() < 1e-6);
    }
}
