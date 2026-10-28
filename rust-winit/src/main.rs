use std::io;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Instant;

use flight_sdk::{Projection, Ref, RigidBody2D, add_node_child, remove_node_children};
use glutin::config::{Config, ConfigTemplateBuilder, GetGlConfig};
use glutin::context::{
    ContextApi, ContextAttributesBuilder, NotCurrentContext, PossiblyCurrentContext, Version,
};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::prelude::*;
use glutin::surface::{GlSurface, Surface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use horse_stacker_rust_winit::assets::asset_path;
use horse_stacker_rust_winit::game::{
    FIXED_STEP_LIMIT, Game, GameMode, GamePhase, HORSE_DROP_ELAPSED_MS, HORSE_REST_ANGULAR,
    HORSE_REST_LINEAR, HORSE_REST_SECONDS, PlacementOutcome, StackObjectSelection, StackedObject,
    create_game,
};
use horse_stacker_rust_winit::physics::{
    HorseStackWorld, PASTURE_MAX_X, PASTURE_MIN_X, PASTURE_TOP_Y, PHYSICS_STEP,
    STACK_CONTACT_GROUND, STACK_CONTACT_PIECE, StackObjectKind, add_stack_object_body,
    create_horse_stack_world, get_settled_stack_height, get_stack_body_contacts,
    get_stack_height_meters, get_stack_object_profile, get_stack_object_vertical_extent,
    get_supported_stack_height, remove_stack_object_body, step_horse_stack,
};
use horse_stacker_rust_winit::scene::{
    CameraRig, SceneGraph, SceneRenderer, StackObjectVisuals, create_camera_rig,
    create_scene_graph, create_scene_renderer, create_stack_object_visuals,
    extract_farm_prop_templates, load_gltf_scene, mount_farm,
};
use raw_window_handle::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

const WINDOW_TITLE: &str = "Horse Stacker — Flight Rust + winit";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let template = ConfigTemplateBuilder::new()
        .with_alpha_size(8)
        .with_depth_size(24)
        .with_stencil_size(8);
    let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes()));
    let mut app = HorseStackerApp::new(template, display_builder).map_err(io::Error::other)?;

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
    Builder(Box<DisplayBuilder>),
    Initialized,
}

struct HorseStackerApp {
    template: ConfigTemplateBuilder,
    display: GlDisplayCreationState,
    renderer: Option<SceneRenderer>,
    surface: Option<AppSurface>,
    context: Option<PossiblyCurrentContext>,
    exit_error: Option<String>,
    game: Game,
    pieces: PieceSequence,
    clock: Instant,
    last_update_ms: f64,
    physics_accumulator: f64,
    physics_world: HorseStackWorld,
    stacked_objects: Vec<StackedObject>,
    scene_graph: SceneGraph,
    camera_rig: CameraRig,
    visuals: StackObjectVisuals,
    sway_clock: f32,
}

impl HorseStackerApp {
    fn new(
        template: ConfigTemplateBuilder,
        display_builder: DisplayBuilder,
    ) -> Result<Self, String> {
        let mut scene_graph = create_scene_graph();
        let farm = load_gltf_scene(asset_path("models/farm"))?;
        let horse = load_gltf_scene(asset_path("models/horse"))?;
        let farm_prop_templates = extract_farm_prop_templates(&farm)?;
        let mut visuals = create_stack_object_visuals();
        visuals.set_templates(farm_prop_templates, horse);
        mount_farm(&farm, &scene_graph.root);
        let mut camera_rig = create_camera_rig();
        camera_rig.update(&mut scene_graph.camera, 1.0, 0.0, 0);

        Ok(Self {
            template,
            display: GlDisplayCreationState::Builder(Box::new(display_builder)),
            renderer: None,
            surface: None,
            context: None,
            exit_error: None,
            game: create_game(StackObjectKind::Horse),
            pieces: PieceSequence::default(),
            clock: Instant::now(),
            last_update_ms: 0.0,
            physics_accumulator: 0.0,
            physics_world: create_horse_stack_world(None),
            stacked_objects: Vec::new(),
            scene_graph,
            camera_rig,
            visuals,
            sway_clock: 0.0,
        })
    }

    fn now_ms(&self) -> f64 {
        self.clock.elapsed().as_secs_f64() * 1_000.0
    }

    fn start_round(&mut self, mode: GameMode) {
        if !matches!(self.game.phase(), GamePhase::Ready | GamePhase::Finished) {
            return;
        }
        let first = self.pieces.next();
        let following = self.pieces.next();
        let now = self.now_ms();
        if self.game.start_round(mode, now, first, following).is_some() {
            self.physics_world = create_horse_stack_world(None);
            self.stacked_objects.clear();
            self.physics_accumulator = 0.0;
            self.last_update_ms = now;
            self.camera_rig.reset_height();
            self.sway_clock = 0.0;
            remove_node_children(&self.scene_graph.stack_layer, None, None);
            self.visuals.clear_pieces();
            for node in self.visuals.instanced_nodes() {
                add_node_child(&self.scene_graph.stack_layer, node);
            }
        }
    }

    fn place(&mut self) {
        let now = self.now_ms();
        if let PlacementOutcome::Placed(selection) = self.game.place(now, now) {
            let kind = selection.kind;
            let bodies = self.stack_bodies();
            let stack_top = get_supported_stack_height(&self.physics_world, &bodies, None);
            let landing_surface = if stack_top > 0.0 {
                stack_top
            } else {
                PASTURE_TOP_Y
            };
            let y = landing_surface + get_stack_object_vertical_extent(kind, 0.0) + 0.08;
            let body = add_stack_object_body(&mut self.physics_world, kind, 0.0, y, 0.0);
            let mut handle = self
                .visuals
                .add_piece(kind, selection.variant_index)
                .expect("loaded models provide every stack-object batch");
            self.visuals
                .set_piece_transform(&mut handle, kind, 0.0, y as f32, 0.0);
            self.stacked_objects
                .push(StackedObject::new(body, kind, handle));
            println!(
                "Placed {}",
                self.visuals.label(kind, selection.variant_index)
            );
        }
    }

    fn update_game(&mut self) {
        let now = self.now_ms();
        let delta_time = ((now - self.last_update_ms) / 1_000.0).clamp(0.0, 0.25);
        self.last_update_ms = now;
        if self.game.is_running() {
            self.physics_accumulator =
                (self.physics_accumulator + delta_time).min(PHYSICS_STEP * FIXED_STEP_LIMIT as f64);
            let mut steps = 0;
            while self.physics_accumulator >= PHYSICS_STEP && steps < FIXED_STEP_LIMIT {
                step_horse_stack(&mut self.physics_world);
                self.physics_accumulator -= PHYSICS_STEP;
                steps += 1;
            }
            self.sway_clock += delta_time as f32 * 1.6;
            self.synchronize_stack_objects(now, delta_time);
            self.visuals.flush();
        }
        let bodies = self.stack_bodies();
        let current_height = get_supported_stack_height(&self.physics_world, &bodies, None);
        let settled_height = get_settled_stack_height(&self.physics_world, &bodies, None);
        let game = &mut self.game;
        let pieces = &mut self.pieces;
        game.update(now, current_height, settled_height, || pieces.next());
        self.camera_rig.update(
            &mut self.scene_graph.camera,
            delta_time as f32,
            self.game.displayed_height() as f32,
            self.game.objects_dropped(),
        );
    }

    fn leave_round(&mut self) {
        self.game.leave_round();
        self.physics_world = create_horse_stack_world(None);
        self.stacked_objects.clear();
        self.physics_accumulator = 0.0;
        self.camera_rig.reset_height();
        self.visuals.clear_pieces();
    }

    fn stack_bodies(&self) -> Vec<Ref<RigidBody2D>> {
        self.stacked_objects
            .iter()
            .filter(|object| !object.lost)
            .map(|object| object.body.clone())
            .collect()
    }

    fn synchronize_stack_objects(&mut self, now_ms: f64, delta_time: f64) {
        let mut lost_bodies = Vec::new();
        let mut newly_dropped_horses = 0;
        for object in &mut self.stacked_objects {
            if object.lost {
                continue;
            }
            let (speed, angular_velocity, x, y, angle, body_index) = {
                let body = object.body.read().unwrap();
                (
                    body.velocity_x.hypot(body.velocity_y),
                    body.angular_velocity,
                    body.x,
                    body.y,
                    body.angle,
                    body.index,
                )
            };
            if f64::from(speed) < HORSE_REST_LINEAR
                && f64::from(angular_velocity.abs()) < HORSE_REST_ANGULAR
            {
                object.rest_seconds += delta_time;
                if object.rest_seconds >= HORSE_REST_SECONDS {
                    object.has_settled = true;
                }
            } else {
                object.rest_seconds = 0.0;
            }

            let contacts = get_stack_body_contacts(&self.physics_world, &object.body);
            if contacts & STACK_CONTACT_PIECE != 0 && object.collided_at_ms == 0.0 {
                object.collided_at_ms = now_ms;
            }
            if !object.touched_ground && contacts & STACK_CONTACT_GROUND != 0 {
                object.touched_ground = true;
                if object.kind == StackObjectKind::Horse
                    && !object.dropped
                    && object.collided_at_ms != 0.0
                    && (object.has_settled
                        || now_ms - object.collided_at_ms > HORSE_DROP_ELAPSED_MS)
                {
                    object.dropped = true;
                    newly_dropped_horses += 1;
                }
            }

            if y < -1.0 || x < PASTURE_MIN_X as f32 - 1.5 || x > PASTURE_MAX_X as f32 + 1.5 {
                object.lost = true;
                if object.kind == StackObjectKind::Horse && !object.dropped {
                    object.dropped = true;
                    newly_dropped_horses += 1;
                }
                lost_bodies.push(object.body.clone());
                continue;
            }

            let carried = ((y - PASTURE_TOP_Y as f32) / 0.9).clamp(0.0, 1.0);
            let sway = carried * carried;
            let sway_phase = self.sway_clock + body_index as f32 * 0.7;
            self.visuals.set_piece_transform(
                &mut object.handle,
                object.kind,
                x + sway_phase.sin() * 0.0016 * sway,
                y,
                angle + (sway_phase * 0.77 + 1.3).sin() * 0.010 * sway,
            );
        }
        for body in lost_bodies {
            remove_stack_object_body(&mut self.physics_world, &body);
        }
        self.stacked_objects.retain(|object| !object.lost);
        for _ in 0..newly_dropped_horses {
            self.game.record_horse_dropped();
        }
    }

    fn update_window_title(&self) {
        let Some(surface) = &self.surface else {
            return;
        };
        let title = match self.game.phase() {
            GamePhase::Loading => format!("{WINDOW_TITLE} — loading"),
            GamePhase::Ready => {
                format!("{WINDOW_TITLE} — Enter: Time Challenge · S: Steady Hands")
            }
            GamePhase::Playing => {
                let mode = mode_label(self.game.mode());
                let clock = match self.game.mode() {
                    GameMode::Time => format!("{:.1}s", self.game.seconds_left_at(self.now_ms())),
                    GameMode::Steady => "no clock".to_owned(),
                };
                let active = self
                    .game
                    .active_kind()
                    .map(|kind| get_stack_object_profile(kind).label)
                    .unwrap_or("waiting");
                let next = get_stack_object_profile(self.game.next_kind()).label;
                format!(
                    "{WINDOW_TITLE} — {mode} · {clock} · {:.2} m · {} placed · Space drops {active} · next {next}",
                    get_stack_height_meters(self.game.displayed_height()),
                    self.game.objects_dropped(),
                )
            }
            GamePhase::Settling => format!("{WINDOW_TITLE} — settling the pile…"),
            GamePhase::Finished => format!(
                "{WINDOW_TITLE} — {:.2} m · Enter: Time · S: Steady",
                get_stack_height_meters(self.game.final_height())
            ),
        };
        surface.window.set_title(&title);
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
            .as_mut()
            .ok_or_else(|| "redraw requested before the Flight renderer was ready".to_owned())?;
        let surface = self
            .surface
            .as_ref()
            .ok_or_else(|| "redraw requested without a window surface".to_owned())?;
        let context = self
            .context
            .as_ref()
            .ok_or_else(|| "redraw requested without a GL context".to_owned())?;

        renderer.draw_scene(&self.scene_graph);
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
                let built = display_builder.as_ref().clone().build(
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
            match create_native_scene_renderer(
                &gl_config.display(),
                size.width.max(1),
                size.height.max(1),
            ) {
                Ok(renderer) => self.renderer = Some(renderer),
                Err(error) => {
                    self.fail(event_loop, error);
                    return;
                }
            }
        } else if let Some(renderer) = self.renderer.as_mut()
            && let Err(error) = renderer.resize(size.width.max(1), size.height.max(1))
        {
            self.fail(event_loop, error);
            return;
        }
        set_camera_aspect(&mut self.scene_graph, size.width.max(1), size.height.max(1));

        if self.game.phase() == GamePhase::Loading {
            self.game.mark_ready();
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
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        state: ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } => match logical_key.as_ref() {
                Key::Named(NamedKey::Escape) => event_loop.exit(),
                Key::Named(NamedKey::Enter) => self.start_round(GameMode::Time),
                Key::Character(value) if value.eq_ignore_ascii_case("s") => {
                    self.start_round(GameMode::Steady);
                }
                Key::Character(" ") => self.place(),
                Key::Named(NamedKey::Backspace) => self.leave_round(),
                _ => {}
            },
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.place(),
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                if let (Some(surface), Some(context)) = (&self.surface, &self.context) {
                    surface.gl_surface.resize(
                        context,
                        NonZeroU32::new(size.width).expect("positive width"),
                        NonZeroU32::new(size.height).expect("positive height"),
                    );
                }
                if let Some(renderer) = self.renderer.as_mut()
                    && let Err(error) = renderer.resize(size.width, size.height)
                {
                    self.fail(event_loop, error);
                    return;
                }
                set_camera_aspect(&mut self.scene_graph, size.width, size.height);
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
        self.update_game();
        self.update_window_title();
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

fn create_native_scene_renderer<D: GlDisplay>(
    display: &D,
    width: u32,
    height: u32,
) -> Result<SceneRenderer, String> {
    let gl = Rc::new(unsafe {
        glow::Context::from_loader_function_cstr(|symbol| display.get_proc_address(symbol))
    });
    create_scene_renderer(gl, width, height)
}

fn set_camera_aspect(scene_graph: &mut SceneGraph, width: u32, height: u32) {
    if let Projection::Perspective(projection) = &scene_graph.camera.projection {
        projection.write().unwrap().aspect = width as f32 / height as f32;
    }
}

#[derive(Default)]
struct PieceSequence {
    cursor: usize,
    chicken_cursor: usize,
}

impl PieceSequence {
    fn next(&mut self) -> StackObjectSelection {
        const PIECES: [StackObjectKind; 20] = [
            StackObjectKind::Horse,
            StackObjectKind::Hay,
            StackObjectKind::Hay,
            StackObjectKind::Chickens,
            StackObjectKind::Hay,
            StackObjectKind::Cow,
            StackObjectKind::Horse,
            StackObjectKind::Hay,
            StackObjectKind::Chickens,
            StackObjectKind::Hay,
            StackObjectKind::Horse,
            StackObjectKind::Hay,
            StackObjectKind::Hay,
            StackObjectKind::Chickens,
            StackObjectKind::Cow,
            StackObjectKind::Hay,
            StackObjectKind::Horse,
            StackObjectKind::Hay,
            StackObjectKind::Chickens,
            StackObjectKind::Hay,
        ];
        let kind = PIECES[self.cursor % PIECES.len()];
        self.cursor += 1;
        let variant_index = if kind == StackObjectKind::Chickens {
            let variant_index = self.chicken_cursor % 2;
            self.chicken_cursor += 1;
            variant_index
        } else {
            0
        };
        StackObjectSelection {
            kind,
            variant_index,
        }
    }
}

const fn mode_label(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Time => "Time Challenge",
        GameMode::Steady => "Steady Hands",
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
