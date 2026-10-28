//! One-time world construction, corresponding to `scene/sceneGraph.ts`.

use std::sync::{Arc, RwLock};

use flight_sdk::{
    AmbientLight, Camera3D, DirectionalLight, DirectionalLightOptions, Material3D, Mesh,
    Node3DFields, NodeRef, OrthographicProjection, PointLight, Projection, Ref, Scene3DLightsLike,
    VertexAttribute, VertexAttributeLayout, VertexColorMaterial, VertexFormat, VertexSemantic,
    add_node_child, configure_directional_shadow_camera_3d_tight_fit, convert_mesh_geometry_layout,
    create_aabb, create_ambient_light, create_camera_3d, create_directional_light, create_mesh,
    create_node3d, create_orthographic_projection, create_perspective_projection,
    create_point_light, create_scene_3d_lights, create_sphere_mesh_geometry, create_vector3,
    get_mesh_geometry_vertex_count, get_mesh_geometry_vertex_position, normalize_vector3,
    publish_node3d, set_mesh_geometry_vertex_color0, srgb_channel_to_linear,
};

use crate::game::{STACK_X, STACK_Z};

pub struct SceneGraph {
    pub root: NodeRef,
    pub stack_layer: NodeRef,
    pub preview_layer: NodeRef,
    pub sky_dome: Ref<Mesh>,
    pub camera: Camera3D,
    pub shadow_camera: Camera3D,
    pub directional_light: Ref<DirectionalLight>,
    pub indicator_light: Ref<PointLight>,
    pub lights: Scene3DLightsLike,
}

const SKY_RADIUS: f32 = 40.0;
const SKY_STOPS: [(f32, u32); 4] = [
    (0.0, 0x2b8ce0),
    (0.42, 0x49a6ea),
    (0.74, 0x74c1f1),
    (1.0, 0xa6dbf8),
];

pub fn create_scene_graph() -> SceneGraph {
    let root = create_group(None);
    let sky_dome = create_sky_dome();
    let sky_node: NodeRef = sky_dome.clone();
    add_node_child(&root, &sky_node);

    let stack_layer = create_group(Some("horse-stack"));
    add_node_child(&root, &stack_layer);
    let preview_layer = create_group(Some("landing-preview-layer"));

    let mut camera = create_camera_3d();
    camera.far = 90.0;
    camera.near = 0.1;
    let perspective = create_perspective_projection();
    {
        let mut perspective = perspective.write().unwrap();
        perspective.aspect = 1.0;
        perspective.fov_y = std::f32::consts::PI / 5.4;
    }
    camera.projection = Projection::Perspective(perspective);

    let mut sun_direction = create_vector3(-0.75, -1.0, -0.5);
    let raw_sun_direction = sun_direction.clone();
    normalize_vector3(&mut sun_direction, &raw_sun_direction);
    let directional_light = create_directional_light(Some(&DirectionalLightOptions {
        casts_shadow: Some(true),
        color: Some(0xfff1d4ff),
        direction: Some(sun_direction.clone()),
        intensity: Some(2.8),
        normal_bias: Some(0.65),
        pcf_radius: Some(1.0),
        shadow_bias: Some(0.001),
        ..DirectionalLightOptions::default()
    }));
    let directional_light = Arc::new(RwLock::new(directional_light));

    let mut indicator_light = create_point_light();
    indicator_light.color = 0xffd56aff;
    indicator_light.intensity = 0.0;
    indicator_light.position = create_vector3(STACK_X as f32 + 0.16, 0.2, STACK_Z as f32);
    indicator_light.range = 0.68;
    let indicator_light = Arc::new(RwLock::new(indicator_light));

    let mut ambient: AmbientLight = create_ambient_light();
    ambient.color = 0xbdd0b5ff;
    ambient.intensity = 0.72;
    let ambient = Arc::new(RwLock::new(ambient));
    let mut lights = create_scene_3d_lights();
    lights.ambient = Some(ambient);
    lights.directional = Some(directional_light.clone());
    lights.point.push(indicator_light.clone());

    let mut shadow_camera = create_camera_3d();
    shadow_camera.far = 55.0;
    shadow_camera.near = 0.1;
    let orthographic = create_orthographic_projection();
    {
        let mut orthographic: std::sync::RwLockWriteGuard<'_, OrthographicProjection> =
            orthographic.write().unwrap();
        orthographic.half_height = 15.0;
        orthographic.half_width = 9.0;
    }
    shadow_camera.projection = Projection::Orthographic(orthographic);
    configure_directional_shadow_camera_3d_tight_fit(
        &mut shadow_camera,
        &sun_direction,
        &create_aabb(-1.95, -0.7, -4.05, 1.95, 2.25, -0.35),
        Some(1.05),
    );

    SceneGraph {
        root,
        stack_layer,
        preview_layer,
        sky_dome,
        camera,
        shadow_camera,
        directional_light,
        indicator_light,
        lights,
    }
}

fn create_group(name: Option<&str>) -> NodeRef {
    let mut group = create_node3d::<Node3DFields>(None);
    group.name = name.map(str::to_owned);
    publish_node3d(group)
}

fn create_sky_dome() -> Ref<Mesh> {
    let layout = Arc::new(RwLock::new(VertexAttributeLayout {
        attributes: vec![
            VertexAttribute {
                byte_offset: 0,
                format: VertexFormat::Float32x3,
                semantic: VertexSemantic::Position,
            },
            VertexAttribute {
                byte_offset: 12,
                format: VertexFormat::Float32x3,
                semantic: VertexSemantic::Normal,
            },
            VertexAttribute {
                byte_offset: 24,
                format: VertexFormat::Float32x4,
                semantic: VertexSemantic::Tangent,
            },
            VertexAttribute {
                byte_offset: 40,
                format: VertexFormat::Float32x2,
                semantic: VertexSemantic::Uv0,
            },
            VertexAttribute {
                byte_offset: 48,
                format: VertexFormat::Float32x4,
                semantic: VertexSemantic::Color0,
            },
        ],
        stride: 64,
    }));
    let sphere = create_sphere_mesh_geometry(Some(SKY_RADIUS), Some(32), Some(20));
    let mut geometry = convert_mesh_geometry_layout(&sphere, &layout);
    let mut position = create_vector3(0.0, 0.0, 0.0);
    for index in 0..get_mesh_geometry_vertex_count(&geometry) {
        get_mesh_geometry_vertex_position(&mut position, &geometry, index);
        let height = position.y / SKY_RADIUS;
        let [red, green, blue] = sample_sky_gradient((1.0 - height * 1.35).clamp(0.0, 1.0));
        set_mesh_geometry_vertex_color0(&mut geometry, index, red, green, blue, 1.0);
    }

    let mut material = VertexColorMaterial::new();
    material.double_sided = true;
    material.tint = 0xffffffff;
    let material: Ref<dyn Material3D> = Arc::new(RwLock::new(material));
    let dome = create_mesh(
        Arc::new(RwLock::new(geometry)),
        Arc::new(RwLock::new(vec![Some(material)])),
    );
    dome.write().unwrap().name = Some("sky".to_owned());
    dome
}

fn sample_sky_gradient(t: f32) -> [f32; 3] {
    for stops in SKY_STOPS.windows(2) {
        let (previous_at, from) = stops[0];
        let (next_at, to) = stops[1];
        if t > next_at {
            continue;
        }
        let span = next_at - previous_at;
        let amount = if span > 0.0 {
            (t - previous_at) / span
        } else {
            0.0
        };
        return [16, 8, 0].map(|shift| {
            let first = ((from >> shift) & 0xff) as f32;
            let second = ((to >> shift) & 0xff) as f32;
            srgb_channel_to_linear((first + (second - first) * amount) / 255.0)
        });
    }
    [1.0, 1.0, 1.0]
}

#[cfg(test)]
mod tests {
    use super::{create_scene_graph, sample_sky_gradient};
    use flight_sdk::get_node_parent;

    #[test]
    fn creates_the_typescript_scene_layers_and_camera_ranges() {
        let graph = create_scene_graph();
        assert!(get_node_parent(&graph.stack_layer).is_some());
        assert!(get_node_parent(&graph.preview_layer).is_none());
        assert_eq!(graph.camera.near, 0.1);
        assert_eq!(graph.camera.far, 90.0);
        assert!(graph.directional_light.read().unwrap().casts_shadow);
    }

    #[test]
    fn sky_gradient_runs_from_deep_to_pale_blue() {
        let zenith = sample_sky_gradient(0.0);
        let horizon = sample_sky_gradient(1.0);
        assert!(horizon[0] > zenith[0]);
        assert!(horizon[1] > zenith[1]);
        assert!(horizon[2] > zenith[2]);
    }
}
