//! Native glTF loading and farm mounting, corresponding to `scene/modelLoader.ts`.

use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};

use flight_sdk::{
    ArrayBufferViewKind, GltfExternalBuffer, GltfExternalBufferView, GltfImportOptions,
    GltfJsonSource, GltfSource, Node3DFields, NodeRef, Record, Ref, Scene3D, add_node_child,
    create_array_buffer_from_bytes, create_array_buffer_view, create_node3d,
    create_scene3d_from_gltf, invalidate_node_local_transform, publish_node3d, set_record_value,
};

use crate::data::FARM_PROP_SCENE_SCALE;

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

#[cfg(test)]
mod tests {
    use super::{load_gltf_scene, mount_farm};
    use crate::assets::asset_path;
    use flight_sdk::{Node3DFields, NodeRef, create_node3d, get_node_children, publish_node3d};

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
}
