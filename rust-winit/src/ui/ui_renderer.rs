//! Flight 2D composition over the native GL frame, corresponding to `ui/uiRenderer.ts`.

use std::rc::Rc;

use flight_sdk::{
    GlRenderState, GlTextureRenderTarget, Matrix, NodeRef, RenderProxies2D, RenderTargetClear,
    RenderTargetColorSpace, RenderTargetDepth, RenderTargetDescriptor, RenderTargetFormat,
    begin_gl_render_pass, create_gl_render_state, create_gl_texture_render_target, create_matrix,
    destroy_gl_render_state, destroy_gl_texture_render_target, end_gl_render_pass,
    gl_scene2d_render_preset, prepare_scene_2d_render, present_gl_render_target, render_gl_scene2d,
};

pub struct UiRenderer {
    pub state: GlRenderState,
    proxies: RenderProxies2D<GlRenderState>,
    target: Option<GlTextureRenderTarget>,
    device_transform: Matrix,
    pixel_ratio: f32,
}

pub fn create_ui_renderer(screen_state: &GlRenderState, pixel_ratio: f32) -> UiRenderer {
    let state = create_gl_render_state(
        Rc::clone(&screen_state.context_state),
        Some(&gl_scene2d_render_preset()),
    );
    UiRenderer {
        state,
        proxies: RenderProxies2D::default(),
        target: None,
        device_transform: create_matrix(pixel_ratio, 0.0, 0.0, pixel_ratio, 0.0, 0.0),
        pixel_ratio,
    }
}

impl UiRenderer {
    pub fn resize(&mut self, width: f32, height: f32, next_pixel_ratio: f32) -> Result<(), String> {
        self.pixel_ratio = next_pixel_ratio;
        self.device_transform =
            create_matrix(next_pixel_ratio, 0.0, 0.0, next_pixel_ratio, 0.0, 0.0);
        if let Some(mut target) = self.target.take() {
            destroy_gl_texture_render_target(&mut target);
        }
        self.target = create_gl_texture_render_target(
            &self.state,
            &RenderTargetDescriptor {
                width: (width * next_pixel_ratio).round().max(1.0),
                height: (height * next_pixel_ratio).round().max(1.0),
                format: Some(RenderTargetFormat::Rgba8),
                color_attachments: None,
                color_formats: None,
                sample_count: Some(1.0),
                depth: Some(RenderTargetDepth::None),
                color_space: Some(RenderTargetColorSpace::Srgb),
            },
            None,
        )?;
        Ok(())
    }

    pub fn render(&mut self, root: &NodeRef) -> Result<(), String> {
        let Some(target) = &self.target else {
            return Ok(());
        };
        if !prepare_scene_2d_render(&mut self.state, &mut self.proxies, root) {
            return Ok(());
        }
        let pass = begin_gl_render_pass(
            &self.state,
            target,
            Some(&RenderTargetClear {
                color: Some([0.0, 0.0, 0.0, 0.0]),
                ..RenderTargetClear::default()
            }),
            None,
        )?;
        render_gl_scene2d(
            &mut self.state,
            &mut self.proxies,
            &pass,
            root,
            Some(&self.device_transform),
        );
        end_gl_render_pass(&self.state, &pass)?;
        present_gl_render_target(&mut self.state, target, None)
    }
}

impl Drop for UiRenderer {
    fn drop(&mut self) {
        if let Some(mut target) = self.target.take() {
            destroy_gl_texture_render_target(&mut target);
        }
        destroy_gl_render_state(&mut self.state, &mut self.proxies);
    }
}
