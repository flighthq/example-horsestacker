//! Native GL frame orchestration, corresponding to `scene/sceneRenderer.ts`.

use std::rc::Rc;

use flight_sdk::{
    GlRenderState, GlTextureRenderTarget, RenderProxies2D, RenderTargetColorSpace,
    RenderTargetDepth, RenderTargetDescriptor, RenderTargetFormat, add_node_child_at,
    create_gl_context_state, create_gl_render_state, create_gl_texture_render_target,
    destroy_gl_render_state, destroy_gl_texture_render_target, get_node_parent,
    gl_scene3d_render_preset, present_gl_scene3d, remove_node_child, render_gl_scene3d_shadow_map,
    resize_gl_texture_render_target,
};

use super::SceneGraph;

pub struct SceneRenderer {
    pub render_state: GlRenderState,
    target: GlTextureRenderTarget,
}

pub fn create_scene_renderer(
    gl: Rc<glow::Context>,
    width: u32,
    height: u32,
) -> Result<SceneRenderer, String> {
    let context_state = create_gl_context_state(gl)?;
    let options = gl_scene3d_render_preset();
    let render_state = create_gl_render_state(context_state, Some(&options));
    let target = create_gl_texture_render_target(
        &render_state,
        &RenderTargetDescriptor {
            width: width as f32,
            height: height as f32,
            format: Some(RenderTargetFormat::Rgba16F),
            color_attachments: None,
            color_formats: None,
            sample_count: Some(4.0),
            depth: Some(RenderTargetDepth::DepthStencil),
            color_space: Some(RenderTargetColorSpace::Linear),
        },
        None,
    )?
    .ok_or_else(|| "Flight could not allocate the scene render target".to_owned())?;
    Ok(SceneRenderer {
        render_state,
        target,
    })
}

impl SceneRenderer {
    pub fn draw_scene(&mut self, scene_graph: &SceneGraph) {
        let preview_parent = get_node_parent(&scene_graph.preview_layer);
        let sky_node = scene_graph.sky_dome.clone();
        let sky_node: flight_sdk::NodeRef = sky_node;
        let sky_parent = get_node_parent(&sky_node);
        if let Some(parent) = &preview_parent {
            remove_node_child(parent, &scene_graph.preview_layer);
        }
        if let Some(parent) = &sky_parent {
            remove_node_child(parent, &sky_node);
        }
        {
            let directional_light = scene_graph.directional_light.read().unwrap();
            render_gl_scene3d_shadow_map(
                &mut self.render_state,
                &scene_graph.root,
                &scene_graph.shadow_camera,
                Some(&directional_light),
            );
        }
        if let Some(parent) = &preview_parent {
            add_node_child_at(parent, &scene_graph.preview_layer, 0);
        }
        if let Some(parent) = &sky_parent {
            add_node_child_at(parent, &sky_node, 0);
        }
        present_gl_scene3d(
            &mut self.render_state,
            &mut self.target,
            &scene_graph.root,
            &scene_graph.camera,
            &scene_graph.lights,
        );
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<bool, String> {
        if self.target.axes.width == width && self.target.axes.height == height {
            return Ok(false);
        }
        resize_gl_texture_render_target(
            &self.render_state,
            &mut self.target,
            width as f32,
            height as f32,
        )?;
        Ok(true)
    }
}

impl Drop for SceneRenderer {
    fn drop(&mut self) {
        destroy_gl_texture_render_target(&mut self.target);
        let mut proxies = RenderProxies2D::<GlRenderState>::default();
        destroy_gl_render_state(&mut self.render_state, &mut proxies);
    }
}
