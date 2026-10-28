//! Native GL frame orchestration, corresponding to `scene/sceneRenderer.ts`.

use std::rc::Rc;

use flight_sdk::{
    BlurEffect, EffectOperation, EffectRef, EffectStateOptions, GlEffectState, GlRenderState,
    GlRenderTarget, Ref, RenderProxies2D, RenderTargetClear, RenderTargetColorSpace,
    RenderTargetDepth, RenderTargetFormat, VignetteEffect, add_node_child_at, begin_gl_effect_pass,
    create_blur_effect, create_gl_context_state, create_gl_effect_state, create_gl_render_state,
    create_vignette_effect, destroy_gl_effect_state, destroy_gl_render_state, end_gl_effect_pass,
    get_node_parent, gl_scene3d_render_preset, register_gl_blur_effect,
    register_gl_particle_emitter3d_pass, register_gl_vignette_effect, remove_node_child,
    render_gl_scene3d, render_gl_scene3d_shadow_map, resize_gl_texture_render_target,
    resolve_gl_texture_render_target,
};
use glow::HasContext;

use super::SceneGraph;

const BACKDROP_BLUR_MAX: f32 = 13.0;

pub struct SceneRenderer {
    pub render_state: GlRenderState,
    pub pipeline: GlEffectState,
    backdrop_blur_effect: Ref<BlurEffect>,
    backdrop_vignette_effect: Ref<VignetteEffect>,
    backdrop_effects_at: i32,
    native_vertex_array: glow::NativeVertexArray,
}

pub fn create_scene_renderer(
    gl: Rc<glow::Context>,
    width: u32,
    height: u32,
) -> Result<SceneRenderer, String> {
    // WebGL supplies a default vertex array. Desktop core GL deliberately does not: binding an
    // element buffer or defining an attribute while VAO 0 is current is INVALID_OPERATION. Flight's
    // shared quad setup and its 2D renderers use the WebGL model, so give those operations a durable
    // native VAO before the context state uploads its shared index buffer.
    // SAFETY: creation and binding happen on the current GL context's event-loop thread.
    let native_vertex_array = unsafe {
        let vertex_array = gl
            .create_vertex_array()
            .map_err(|error| format!("create native compatibility vertex array: {error}"))?;
        gl.bind_vertex_array(Some(vertex_array));
        vertex_array
    };
    let context_state = create_gl_context_state(Rc::clone(&gl)).inspect_err(|_| {
        // SAFETY: the context is still current and no Flight state retained the failed VAO.
        unsafe { gl.delete_vertex_array(native_vertex_array) };
    })?;
    eprintln!("[horse-stacker] GL compatibility: native default VAO installed");
    let options = gl_scene3d_render_preset();
    let mut render_state = create_gl_render_state(context_state, Some(&options));
    register_gl_particle_emitter3d_pass(&mut render_state);
    register_gl_blur_effect(&mut render_state);
    register_gl_vignette_effect(&mut render_state);
    let pipeline = create_gl_effect_state(
        &render_state,
        Some(&EffectStateOptions {
            sample_count: Some(4.0),
            format: Some(RenderTargetFormat::Rgba16F),
            depth: Some(RenderTargetDepth::DepthStencil),
        }),
    );
    let backdrop_blur_effect = create_blur_effect();
    {
        let mut effect = backdrop_blur_effect.write().unwrap();
        effect.blur_x = Some(0.0);
        effect.blur_y = Some(0.0);
    }
    let backdrop_vignette_effect = create_vignette_effect();
    {
        let mut effect = backdrop_vignette_effect.write().unwrap();
        effect.color = Some(0x0d1622ff);
        effect.intensity = Some(0.0);
        effect.radius = Some(0.5);
        effect.softness = Some(0.85);
    }
    eprintln!(
        "[horse-stacker] scene pipeline: {width}x{height} format=Rgba16F samples=4 depth=DepthStencil color_space=Linear particles=registered effects=blur+vignette",
    );
    Ok(SceneRenderer {
        render_state,
        pipeline,
        backdrop_blur_effect,
        backdrop_vignette_effect,
        backdrop_effects_at: -1,
        native_vertex_array,
    })
}

impl SceneRenderer {
    pub fn draw_scene(&mut self, scene_graph: &SceneGraph, backdrop_focus: f32) {
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
        let pass = begin_gl_effect_pass(
            &mut self.render_state,
            &mut self.pipeline,
            Some(&RenderTargetClear {
                color: Some([0.0, 0.0, 0.0, 0.0]),
                depth: Some(1.0),
                ..RenderTargetClear::default()
            }),
            Some(RenderTargetColorSpace::Linear),
        )
        .unwrap_or_else(|error| flight_sdk::throw_error(&error));
        render_gl_scene3d(
            &mut self.render_state,
            &pass,
            self.pipeline
                .scene_target
                .as_mut()
                .map(|target| target as &mut dyn GlRenderTarget),
            &scene_graph.root,
            &scene_graph.camera,
            &scene_graph.lights,
            None,
        );
        // At the pinned Flight revision, end_gl_render_pass's snapshot resolve does not populate a
        // multisampled target on native desktop GL. Resolve the concrete effect target while the
        // scene pass is still active, before end_gl_effect_pass samples it for effects/presentation.
        // The call is a no-op for a single-sample fallback.
        if let Some(target) = self.pipeline.scene_target.as_ref() {
            resolve_gl_texture_render_target(&self.render_state, target);
        }
        let active_effects = self.get_backdrop_effects(backdrop_focus);
        end_gl_effect_pass(
            &mut self.render_state,
            &pass,
            &mut self.pipeline,
            &active_effects,
        )
        .unwrap_or_else(|error| flight_sdk::throw_error(&error));
    }

    fn get_backdrop_effects(&mut self, focus: f32) -> Vec<EffectOperation> {
        if focus <= 0.002 {
            return Vec::new();
        }
        let quantized = (focus.clamp(0.0, 1.0) * 60.0).round() as i32;
        if quantized != self.backdrop_effects_at {
            self.backdrop_effects_at = quantized;
            let amount = quantized as f32 / 60.0;
            {
                let mut effect = self.backdrop_blur_effect.write().unwrap();
                effect.blur_x = Some(BACKDROP_BLUR_MAX * amount);
                effect.blur_y = Some(BACKDROP_BLUR_MAX * amount);
            }
            self.backdrop_vignette_effect.write().unwrap().intensity = Some(0.7 * amount);
        }
        let blur: EffectRef = self.backdrop_blur_effect.clone();
        let vignette: EffectRef = self.backdrop_vignette_effect.clone();
        vec![
            EffectOperation::Effect(blur),
            EffectOperation::Effect(vignette),
        ]
    }

    pub fn bind_native_vertex_array(&self) {
        // Fullscreen and mesh passes own temporary VAOs and restore VAO 0. Rebind the compatibility
        // VAO before Flight's 2D renderers, whose buffer/attribute setup follows WebGL semantics.
        // SAFETY: the VAO belongs to this current context and is used only on the event-loop thread.
        unsafe {
            self.render_state
                .gl
                .bind_vertex_array(Some(self.native_vertex_array));
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<bool, String> {
        let Some(target) = self.pipeline.scene_target.as_mut() else {
            return Ok(false);
        };
        if target.axes.width == width && target.axes.height == height {
            return Ok(false);
        }
        resize_gl_texture_render_target(&self.render_state, target, width as f32, height as f32)?;
        Ok(true)
    }
}

impl Drop for SceneRenderer {
    fn drop(&mut self) {
        destroy_gl_effect_state(&self.render_state, &mut self.pipeline);
        let mut proxies = RenderProxies2D::<GlRenderState>::default();
        destroy_gl_render_state(&mut self.render_state, &mut proxies);
        // SAFETY: native shutdown runs while the owning context is current.
        unsafe {
            self.render_state.gl.bind_vertex_array(None);
            self.render_state
                .gl
                .delete_vertex_array(self.native_vertex_array);
        }
    }
}
