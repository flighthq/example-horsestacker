use std::io;
use std::num::NonZeroU32;
use std::rc::Rc;

use flight_sdk::{
    GlRenderState, GlScreenRenderTarget, RenderProxies2D, RenderTargetClear, begin_gl_render_pass,
    create_gl_context_state, create_gl_render_state, create_gl_screen_render_target,
    destroy_gl_render_state, end_gl_render_pass,
};
use glutin::config::{Config, ConfigTemplateBuilder, GetGlConfig};
use glutin::context::{
    ContextApi, ContextAttributesBuilder, NotCurrentContext, PossiblyCurrentContext, Version,
};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::prelude::*;
use glutin::surface::{GlSurface, Surface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use raw_window_handle::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

const WINDOW_TITLE: &str = "Horse Stacker — Flight Rust + winit";
const CLEAR_COLOR: [f32; 4] = [0.055, 0.09, 0.045, 1.0];

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let template = ConfigTemplateBuilder::new()
        .with_alpha_size(8)
        .with_depth_size(24)
        .with_stencil_size(8);
    let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes()));
    let mut app = HorseStackerApp::new(template, display_builder);

    event_loop.run_app(&mut app)?;

    if let Some(error) = app.exit_error {
        return Err(io::Error::other(error).into());
    }
    Ok(())
}

fn window_attributes() -> WindowAttributes {
    Window::default_attributes()
        .with_title(WINDOW_TITLE)
        .with_inner_size(LogicalSize::new(1280.0, 720.0))
        .with_min_inner_size(LogicalSize::new(640.0, 360.0))
}

enum GlDisplayCreationState {
    Builder(DisplayBuilder),
    Initialized,
}

struct HorseStackerApp {
    template: ConfigTemplateBuilder,
    display: GlDisplayCreationState,
    renderer: Option<FlightRenderer>,
    surface: Option<AppSurface>,
    context: Option<PossiblyCurrentContext>,
    exit_error: Option<String>,
}

impl HorseStackerApp {
    fn new(template: ConfigTemplateBuilder, display_builder: DisplayBuilder) -> Self {
        Self {
            template,
            display: GlDisplayCreationState::Builder(display_builder),
            renderer: None,
            surface: None,
            context: None,
            exit_error: None,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl Into<String>) {
        let error = error.into();
        eprintln!("Horse Stacker failed: {error}");
        self.exit_error = Some(error);
        event_loop.exit();
    }

    fn redraw(&mut self) -> Result<(), String> {
        let renderer = self
            .renderer
            .as_ref()
            .ok_or_else(|| "redraw requested before the Flight renderer was ready".to_owned())?;
        let surface = self
            .surface
            .as_ref()
            .ok_or_else(|| "redraw requested without a window surface".to_owned())?;
        let context = self
            .context
            .as_ref()
            .ok_or_else(|| "redraw requested without a GL context".to_owned())?;

        renderer.draw()?;
        surface.window.pre_present_notify();
        surface
            .gl_surface
            .swap_buffers(context)
            .map_err(|error| format!("swap buffers: {error}"))
    }
}

impl ApplicationHandler for HorseStackerApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let (window, gl_config) = match &self.display {
            GlDisplayCreationState::Builder(display_builder) => {
                let built = display_builder.clone().build(
                    event_loop,
                    self.template.clone(),
                    pick_gl_config,
                );
                let (window, gl_config) = match built {
                    Ok((Some(window), gl_config)) => (window, gl_config),
                    Ok((None, _)) => {
                        self.fail(event_loop, "the GL display did not create a window");
                        return;
                    }
                    Err(error) => {
                        self.fail(event_loop, format!("create GL display: {error}"));
                        return;
                    }
                };
                self.display = GlDisplayCreationState::Initialized;
                self.context = match create_gl_context(&window, &gl_config) {
                    Ok(context) => Some(context.treat_as_possibly_current()),
                    Err(error) => {
                        self.fail(event_loop, error);
                        return;
                    }
                };
                (window, gl_config)
            }
            GlDisplayCreationState::Initialized => {
                let gl_config = self
                    .context
                    .as_ref()
                    .expect("initialized display must retain its context")
                    .config();
                let window = match glutin_winit::finalize_window(
                    event_loop,
                    window_attributes(),
                    &gl_config,
                ) {
                    Ok(window) => window,
                    Err(error) => {
                        self.fail(event_loop, format!("recreate window: {error}"));
                        return;
                    }
                };
                (window, gl_config)
            }
        };

        let attributes = match window.build_surface_attributes(Default::default()) {
            Ok(attributes) => attributes,
            Err(error) => {
                self.fail(event_loop, format!("create surface attributes: {error}"));
                return;
            }
        };
        let gl_surface = match unsafe {
            gl_config
                .display()
                .create_window_surface(&gl_config, &attributes)
        } {
            Ok(surface) => surface,
            Err(error) => {
                self.fail(event_loop, format!("create window surface: {error}"));
                return;
            }
        };

        let context = self
            .context
            .as_ref()
            .expect("a context is created before its surface");
        if let Err(error) = context.make_current(&gl_surface) {
            self.fail(event_loop, format!("make GL context current: {error}"));
            return;
        }
        let _ = gl_surface.set_swap_interval(
            context,
            SwapInterval::Wait(NonZeroU32::new(1).expect("one is non-zero")),
        );

        let size = window.inner_size();
        if self.renderer.is_none() {
            match FlightRenderer::new(&gl_config.display(), size.width.max(1), size.height.max(1)) {
                Ok(renderer) => self.renderer = Some(renderer),
                Err(error) => {
                    self.fail(event_loop, error);
                    return;
                }
            }
        } else if let Some(renderer) = self.renderer.as_mut() {
            renderer.resize(size.width.max(1), size.height.max(1));
        }

        self.surface = Some(AppSurface { gl_surface, window });
        if let Some(surface) = &self.surface {
            surface.window.request_redraw();
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.surface = None;
        if let Some(context) = self.context.take() {
            self.context = Some(
                context
                    .make_not_current()
                    .expect("suspending a current GL context")
                    .treat_as_possibly_current(),
            );
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => event_loop.exit(),
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                if let (Some(surface), Some(context)) = (&self.surface, &self.context) {
                    surface.gl_surface.resize(
                        context,
                        NonZeroU32::new(size.width).expect("positive width"),
                        NonZeroU32::new(size.height).expect("positive height"),
                    );
                }
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
                if let Some(surface) = &self.surface {
                    surface.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.redraw() {
                    self.fail(event_loop, error);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(surface) = &self.surface {
            surface.window.request_redraw();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // Keep the GL context current while renderer-owned resources are released.
        self.renderer = None;
        self.surface = None;
        self.context = None;
    }
}

struct AppSurface {
    gl_surface: Surface<WindowSurface>,
    // This must outlive the surface created from its raw window handle.
    window: Window,
}

struct FlightRenderer {
    state: GlRenderState,
    screen: GlScreenRenderTarget,
}

impl FlightRenderer {
    fn new<D: GlDisplay>(display: &D, width: u32, height: u32) -> Result<Self, String> {
        let gl = Rc::new(unsafe {
            glow::Context::from_loader_function_cstr(|symbol| display.get_proc_address(symbol))
        });
        let context_state = create_gl_context_state(Rc::clone(&gl))?;
        let state = create_gl_render_state(context_state, None);
        let screen = create_gl_screen_render_target(gl, width, height);
        Ok(Self { state, screen })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.screen.width = width;
        self.screen.height = height;
    }

    fn draw(&self) -> Result<(), String> {
        let clear = RenderTargetClear {
            color: Some(CLEAR_COLOR),
            depth: Some(1.0),
            stencil: Some(0),
            ..RenderTargetClear::default()
        };
        let pass = begin_gl_render_pass(&self.state, &self.screen, Some(&clear), None)?;
        end_gl_render_pass(&self.state, &pass)
    }
}

impl Drop for FlightRenderer {
    fn drop(&mut self) {
        let mut proxies = RenderProxies2D::<GlRenderState>::default();
        destroy_gl_render_state(&mut self.state, &mut proxies);
    }
}

fn create_gl_context(window: &Window, gl_config: &Config) -> Result<NotCurrentContext, String> {
    let raw_window_handle = window.window_handle().ok().map(|handle| handle.as_raw());
    // Flight's GL backend mirrors WebGL2, so do not silently accept a GL 2.1/GLES 2 context.
    let context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
        .build(raw_window_handle);
    let gles_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::Gles(Some(Version::new(3, 0))))
        .build(raw_window_handle);
    let display = gl_config.display();

    unsafe {
        display
            .create_context(gl_config, &context_attributes)
            .or_else(|_| display.create_context(gl_config, &gles_attributes))
            .map_err(|error| format!("create a GL 3.3 or GLES 3.0 context: {error}"))
    }
}

fn pick_gl_config(configs: Box<dyn Iterator<Item = Config> + '_>) -> Config {
    configs
        .reduce(|best, candidate| {
            if candidate.num_samples() > best.num_samples() {
                candidate
            } else {
                best
            }
        })
        .expect("the platform must expose at least one GL configuration")
}
