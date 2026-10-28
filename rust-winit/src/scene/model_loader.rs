//! Native glTF loading and farm mounting, corresponding to `scene/modelLoader.ts`.

use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};

use flight_sdk::node::contract::{
    get_node_local_matrix4, get_node_transform3d, is_node_local_matrix4_detached,
    set_node_local_matrix4, set_node_transform3d,
};
use flight_sdk::{
    ArrayBufferViewKind, EulerOrder, GltfExternalBuffer, GltfExternalBufferView, GltfImportOptions,
    GltfJsonSource, GltfSource, IndexBuffer, IndexBufferU16, IndexBufferU32, Mesh, MeshGeometry,
    MeshSubset, Node3DFields, NodeRef, PrimitiveTopology, Record, Ref, Scene3D, add_node_child,
    as_node3d_ref, clone_mesh, clone_mesh_geometry, compact_mesh_geometry_vertices,
    create_array_buffer_from_bytes, create_array_buffer_view, create_node3d,
    create_scene3d_from_gltf, create_transform3d, find_node_by_name,
    invalidate_node_local_transform, publish_node3d, refresh_mesh_geometry_bounds,
    set_mesh_geometry_subsets, set_quaternion_from_euler, set_record_value,
};

use crate::data::{
    FARM_PROP_SCENE_SCALE, FARM_PROP_VARIANTS, FarmPropKind, FarmPropPartSpec,
    FarmPropTriangleFilter, select_farm_prop_triangle_indices,
};

#[derive(Default)]
pub struct FarmPropTemplates {
    pub hay: Vec<NodeRef>,
    pub cow: Vec<NodeRef>,
    pub chickens: Vec<NodeRef>,
}

impl FarmPropTemplates {
    pub fn get(&self, kind: FarmPropKind) -> &[NodeRef] {
        match kind {
            FarmPropKind::Hay => &self.hay,
            FarmPropKind::Cow => &self.cow,
            FarmPropKind::Chickens => &self.chickens,
        }
    }
}

/// Loads the same `scene.gltf` + `scene.bin` pair fetched by the web implementation.
pub fn load_gltf_scene(base_path: impl AsRef<Path>) -> Result<Scene3D, String> {
    let base_path = base_path.as_ref();
    let document_path = base_path.join("scene.gltf");
    let buffer_path = base_path.join("scene.bin");
    let document = fs::read_to_string(&document_path)
        .map_err(|error| format!("read {}: {error}", document_path.display()))?;
    let bytes = fs::read(&buffer_path)
        .map_err(|error| format!("read {}: {error}", buffer_path.display()))?;
    let byte_length = bytes.len();
    let buffer = create_array_buffer_from_bytes(bytes.into_boxed_slice());
    let view = create_array_buffer_view(&buffer, 0, byte_length, ArrayBufferViewKind::Uint8Array)
        .ok_or_else(|| format!("create byte view for {}", buffer_path.display()))?;

    let mut external_buffers = Record::default();
    set_record_value(
        &mut external_buffers,
        "scene.bin".to_owned(),
        GltfExternalBuffer::View(GltfExternalBufferView { view }),
    );
    let external_buffers = Arc::new(RwLock::new(external_buffers));
    let diagnostics: Ref<Vec<flight_sdk::ImportDiagnostic>> = Arc::new(RwLock::new(Vec::new()));
    let options = GltfImportOptions {
        base_path: Some(format!("{}/", base_path.display())),
        extension_handlers: None,
        external_buffers: Some(external_buffers),
        draco_registry: None,
    };
    let source = GltfSource::Json(GltfJsonSource { source: &document });
    let scene = create_scene3d_from_gltf(&source, Some(&diagnostics), Some(&options));

    let diagnostics = diagnostics.read().unwrap();
    if !diagnostics.is_empty() {
        eprintln!(
            "Flight imported {} with {} diagnostic(s)",
            base_path.display(),
            diagnostics.len()
        );
    }
    Ok(scene)
}

pub fn mount_farm(model: &Scene3D, scene_root: &NodeRef) {
    let wrapper = publish_node3d(create_node3d::<Node3DFields>(None));
    {
        let mut wrapper = wrapper.write().unwrap();
        wrapper.scale.x = FARM_PROP_SCENE_SCALE;
        wrapper.scale.y = FARM_PROP_SCENE_SCALE;
        wrapper.scale.z = FARM_PROP_SCENE_SCALE;
        wrapper.position.x = 0.5;
        wrapper.position.y = -0.043;
        wrapper.position.z = -2.2;
    }
    let wrapper_node: NodeRef = wrapper;
    invalidate_node_local_transform(&wrapper_node);
    let model_root: NodeRef = model.root.clone();
    add_node_child(&wrapper_node, &model_root);
    add_node_child(scene_root, &wrapper_node);
}

pub fn extract_farm_prop_templates(farm: &Scene3D) -> Result<FarmPropTemplates, String> {
    let mut result = FarmPropTemplates::default();
    let farm_root: NodeRef = farm.root.clone();
    for kind in [FarmPropKind::Hay, FarmPropKind::Cow, FarmPropKind::Chickens] {
        let mut templates = Vec::new();
        for (variant_index, spec) in FARM_PROP_VARIANTS.get(kind).iter().enumerate() {
            let template = create_group(Some(&format!(
                "{}-{variant_index}-template",
                farm_prop_key(kind)
            )));
            let scale_root = create_group(None);
            {
                let scale = FARM_PROP_SCENE_SCALE * spec.scale_multiplier.unwrap_or(1.0);
                let node3d = as_node3d_ref(&scale_root).expect("farm prop scale root is Node3D");
                let mut scale_root = node3d.write().unwrap();
                let transform = scale_root.node3d_fields_mut();
                transform.scale.x = scale;
                transform.scale.y = scale;
                transform.scale.z = scale;
            }
            invalidate_node_local_transform(&scale_root);

            let axis_root = create_group(None);
            let mut farm_transform = create_transform3d();
            get_node_transform3d(&mut farm_transform, &farm_root);
            set_node_transform3d(&axis_root, &farm_transform);
            if is_node_local_matrix4_detached(&farm_root) {
                set_node_local_matrix4(&axis_root, &get_node_local_matrix4(&farm_root));
            }

            let centered_source = create_group(None);
            {
                let node3d =
                    as_node3d_ref(&centered_source).expect("farm prop center root is Node3D");
                let mut centered_source = node3d.write().unwrap();
                let transform = centered_source.node3d_fields_mut();
                transform.position.x = -spec.center_x;
                transform.position.y = -spec.center_y;
                transform.position.z = -spec.center_z;
            }
            invalidate_node_local_transform(&centered_source);
            for part in spec.parts {
                let source = find_node_by_name(&farm_root, part.node_name)
                    .ok_or_else(|| format!("Farm prop mesh {} was not imported", part.node_name))?;
                let cloned = clone_farm_prop_part(&source, part)?;
                let cloned: NodeRef = cloned;
                add_node_child(&centered_source, &cloned);
            }

            let orientation_root = create_group(None);
            {
                let node3d =
                    as_node3d_ref(&orientation_root).expect("farm prop orientation root is Node3D");
                let mut orientation_root = node3d.write().unwrap();
                set_quaternion_from_euler(
                    &mut orientation_root.node3d_fields_mut().rotation,
                    0.0,
                    0.0,
                    spec.rotation_z.unwrap_or(0.0),
                    EulerOrder::XYZ,
                );
            }
            invalidate_node_local_transform(&orientation_root);
            add_node_child(&orientation_root, &centered_source);
            add_node_child(&axis_root, &orientation_root);
            add_node_child(&scale_root, &axis_root);
            add_node_child(&template, &scale_root);
            templates.push(template);
        }
        match kind {
            FarmPropKind::Hay => result.hay = templates,
            FarmPropKind::Cow => result.cow = templates,
            FarmPropKind::Chickens => result.chickens = templates,
        }
    }
    Ok(result)
}

fn clone_farm_prop_part(source: &NodeRef, part: &FarmPropPartSpec) -> Result<Ref<Mesh>, String> {
    let source = as_node3d_ref(source)
        .ok_or_else(|| format!("Farm prop mesh {} is not a Node3D", part.node_name))?;
    let source = source.read().unwrap();
    let mesh = source
        .as_mesh()
        .ok_or_else(|| format!("Farm prop mesh {} was not imported", part.node_name))?;
    let material_name = mesh
        .materials
        .read()
        .unwrap()
        .first()
        .and_then(Option::as_ref)
        .and_then(|material| material.read().unwrap().material_fields().name.clone());
    if material_name.as_deref() != Some(part.material_name) {
        return Err(format!(
            "Farm prop mesh {} uses {}, expected {}",
            part.node_name,
            material_name.as_deref().unwrap_or("no material"),
            part.material_name,
        ));
    }
    let cloned = clone_mesh(mesh);
    if let Some(filter) = part.filter {
        let geometry =
            filter_farm_prop_geometry(&mesh.geometry.read().unwrap(), filter, part.node_name)?;
        cloned.write().unwrap().geometry = Arc::new(RwLock::new(geometry));
    }
    Ok(cloned)
}

fn filter_farm_prop_geometry(
    source: &MeshGeometry,
    filter: &FarmPropTriangleFilter,
    node_name: &str,
) -> Result<MeshGeometry, String> {
    if source.topology != PrimitiveTopology::TriangleList
        || matches!(&source.indices, IndexBuffer::None)
    {
        return Err(format!(
            "Farm prop mesh {node_name} is not an indexed triangle list"
        ));
    }
    let selected_indices = select_farm_prop_triangle_indices(source, filter);
    if selected_indices.is_empty() {
        return Err(format!(
            "Farm prop mesh {node_name} produced no selected triangles"
        ));
    }

    let mut filtered = clone_mesh_geometry(source);
    filtered.indices = match &source.indices {
        IndexBuffer::U16(_) => IndexBuffer::U16(IndexBufferU16 {
            data: Arc::new(RwLock::new(
                selected_indices
                    .iter()
                    .map(|index| *index as u16)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            )),
        }),
        IndexBuffer::U32(_) => IndexBuffer::U32(IndexBufferU32 {
            data: Arc::new(RwLock::new(selected_indices.into_boxed_slice())),
        }),
        IndexBuffer::None => unreachable!(),
    };
    let index_count = match &filtered.indices {
        IndexBuffer::U16(indices) => indices.data.read().unwrap().len(),
        IndexBuffer::U32(indices) => indices.data.read().unwrap().len(),
        IndexBuffer::None => 0,
    };
    set_mesh_geometry_subsets(
        &mut filtered,
        &[MeshSubset {
            index_count: index_count as u32,
            index_offset: 0,
        }],
    );
    let mut compact = compact_mesh_geometry_vertices(&filtered);
    refresh_mesh_geometry_bounds(&mut compact);
    Ok(compact)
}

fn create_group(name: Option<&str>) -> NodeRef {
    let mut group = create_node3d::<Node3DFields>(None);
    group.name = name.map(str::to_owned);
    publish_node3d(group)
}

const fn farm_prop_key(kind: FarmPropKind) -> &'static str {
    match kind {
        FarmPropKind::Hay => "hay",
        FarmPropKind::Cow => "cow",
        FarmPropKind::Chickens => "chickens",
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_farm_prop_templates, load_gltf_scene, mount_farm};
    use crate::assets::asset_path;
    use crate::data::FARM_PROP_VARIANTS;
    #[cfg(feature = "native")]
    use crate::scene::{create_camera_rig, create_scene_graph};
    #[cfg(feature = "native")]
    use flight_sdk::{
        GlMaterialRenderer, STANDARD_MATERIAL_KIND, create_render_state, gl_scene3d_render_preset,
        prepare_scene_3d_render,
    };
    use flight_sdk::{
        IndexBuffer, Node3DFields, NodeRef, as_node3d_ref, create_node3d, get_node_children,
        publish_node3d,
    };

    #[test]
    fn loads_and_mounts_the_shared_farm_gltf() {
        let farm = load_gltf_scene(asset_path("models/farm")).unwrap();
        let farm_root: NodeRef = farm.root.clone();
        let mut imported = Vec::new();
        get_node_children(&mut imported, &farm_root);
        assert!(!imported.is_empty());

        let root = publish_node3d(create_node3d::<Node3DFields>(None));
        let root: NodeRef = root;
        mount_farm(&farm, &root);
        let mut mounted = Vec::new();
        get_node_children(&mut mounted, &root);
        assert_eq!(mounted.len(), 1);
    }

    #[cfg(feature = "native")]
    #[test]
    fn mounted_farm_and_sky_are_visible_to_the_initial_camera() {
        let farm = load_gltf_scene(asset_path("models/farm")).unwrap();
        let mut scene = create_scene_graph();
        mount_farm(&farm, &scene.root);
        let mut camera_rig = create_camera_rig();
        camera_rig.update(&mut scene.camera, 1.0, 0.0, 0);

        let mut state = create_render_state();
        let prepared = prepare_scene_3d_render(
            &mut state,
            &scene.root,
            &scene.camera,
            &scene.lights,
            Some(16.0 / 9.0),
        );
        assert!(
            prepared.mesh_count > 1,
            "initial camera should see the sky plus farm meshes"
        );
        let subset_count = prepared.visible_meshes[..prepared.mesh_count]
            .iter()
            .map(|node| {
                as_node3d_ref(node).map_or(0, |node| {
                    let node = node.read().unwrap();
                    node.as_mesh()
                        .map_or(0, |mesh| mesh.geometry.read().unwrap().subsets.len())
                })
            })
            .sum::<usize>();
        assert!(
            subset_count > 1,
            "visible meshes must contain drawable subsets"
        );
        let preset = gl_scene3d_render_preset();
        for node in &prepared.visible_meshes[..prepared.mesh_count] {
            let Some(node) = as_node3d_ref(node) else {
                continue;
            };
            let node = node.read().unwrap();
            let Some(mesh) = node.as_mesh() else {
                continue;
            };
            for material in mesh.materials.read().unwrap().iter() {
                let kind = material.as_ref().map_or_else(
                    || STANDARD_MATERIAL_KIND.to_owned(),
                    |material| material.read().unwrap().material_fields().kind.clone(),
                );
                assert!(
                    matches!(
                        preset.gl_registries.material_renderers.entries.get(&kind),
                        Some(GlMaterialRenderer::Mesh(_))
                    ),
                    "visible material kind {kind} must have a 3D GL renderer"
                );
            }
        }
    }

    #[test]
    fn extracts_every_authored_farm_prop_variant() {
        let farm = load_gltf_scene(asset_path("models/farm")).unwrap();
        let templates = extract_farm_prop_templates(&farm).unwrap();
        assert_eq!(templates.hay.len(), 1);
        assert_eq!(templates.cow.len(), 1);
        assert_eq!(templates.chickens.len(), 2);
        for (templates, specs) in [
            (&templates.hay, FARM_PROP_VARIANTS.hay),
            (&templates.cow, FARM_PROP_VARIANTS.cow),
            (&templates.chickens, FARM_PROP_VARIANTS.chickens),
        ] {
            for (template, spec) in templates.iter().zip(specs) {
                assert_eq!(count_triangles(template), spec.expected_triangle_count);
            }
        }
    }

    fn count_triangles(node: &NodeRef) -> usize {
        let own_triangles = as_node3d_ref(node)
            .and_then(|node| {
                let node = node.read().unwrap();
                node.as_mesh()
                    .map(|mesh| match &mesh.geometry.read().unwrap().indices {
                        IndexBuffer::U16(indices) => indices.data.read().unwrap().len() / 3,
                        IndexBuffer::U32(indices) => indices.data.read().unwrap().len() / 3,
                        IndexBuffer::None => 0,
                    })
            })
            .unwrap_or(0);
        let mut children = Vec::new();
        get_node_children(&mut children, node);
        own_triangles + children.iter().map(count_triangles).sum::<usize>()
    }
}
