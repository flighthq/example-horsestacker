//! The native shell: winit events, GL lifecycle, fullscreen, frame scheduling, and UI mapping.
//!
//! The round rules remain in `game/game.rs`, just as `typescript-web/src/main.ts` leaves them in
//! `game/game.ts`. This file assembles the same object graph in dependency order.

use std::io;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Instant;

use flight_sdk::Projection;
use glutin::config::{Config, ConfigTemplateBuilder, GetGlConfig};
use glutin::context::{
    ContextApi, ContextAttributesBuilder, NotCurrentContext, PossiblyCurrentContext, Version,
};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::prelude::*;
use glutin::surface::{GlSurface, Surface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use horse_stacker_rust_winit::assets::asset_path;
use horse_stacker_rust_winit::audio::{AudioManager, create_audio_manager};
use horse_stacker_rust_winit::game::{Game, GameDeps, GameMode, GamePhase, create_game};
use horse_stacker_rust_winit::native_storage::NativePreferences;
use horse_stacker_rust_winit::physics::{StackObjectKind, get_stack_height_meters};
use horse_stacker_rust_winit::scene::{
    CameraRig, LandingIndicator, ParticleEffects, PlayPlanePoint, SceneGraph, SceneRenderer,
    StackObjectVisuals, Windmill, create_camera_rig, create_landing_indicator,
    create_particle_effects, create_scene_graph, create_scene_renderer,
    create_stack_object_visuals, create_windmill, extract_farm_prop_templates, load_gltf_scene,
    mount_farm, sample_farm_terrain, unproject_to_play_plane,
};
use horse_stacker_rust_winit::ui::{GameUi2D, UiButtonId, UiModel, UiScreen, create_game_ui_2d};
use raw_window_handle::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window, WindowAttributes, WindowId};

const WINDOW_TITLE: &str = "Horse Stacker — Flight Rust + winit";
const KEYBOARD_NUDGE: f64 = 0.05;

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

struct InputBounds {
    width: f32,
    height: f32,
}

struct HorseStackerApp {
    template: ConfigTemplateBuilder,
    display: GlDisplayCreationState,
    renderer: Option<SceneRenderer>,
    game_ui: Option<GameUi2D>,
    surface: Option<AppSurface>,
    context: Option<PossiblyCurrentContext>,
    exit_error: Option<String>,

    audio: AudioManager,
    scene_graph: SceneGraph,
    particles: ParticleEffects,
    camera_rig: CameraRig,
    visuals: StackObjectVisuals,
    indicator: LandingIndicator,
    game: Game,
    windmill: Option<Windmill>,
    storage: NativePreferences,

    credits_open: bool,
    pointer_x: f32,
    pointer_y: f32,
    pointer_down: bool,
    backdrop_focus: f64,
    clock: Instant,
    previous_time: f64,
    is_viewer_visible: bool,
    render_requested: bool,
    input_bounds: InputBounds,
    aim_point: PlayPlanePoint,
    pixel_ratio: f32,
    status_copy: String,
    edge_probe_requested: bool,
    forced_screen: Option<UiScreen>,
}

impl HorseStackerApp {
    fn new(
        template: ConfigTemplateBuilder,
        display_builder: DisplayBuilder,
    ) -> Result<Self, String> {
        let storage_path = std::env::current_dir()
            .unwrap_or_default()
            .join(".horse-stacker.preferences");
        let storage = NativePreferences::load(storage_path);
        let game = create_game(&storage);
        let audio = create_audio_manager(asset_path("sounds"));
        let mut scene_graph = create_scene_graph();
        let particles = create_particle_effects(&scene_graph.root);
        let mut camera_rig = create_camera_rig();
        let mut visuals = create_stack_object_visuals();
        let mut indicator = create_landing_indicator(
            scene_graph.preview_layer.clone(),
            scene_graph.indicator_light.clone(),
        );

        // Native model loading is synchronous, but otherwise this is the TS `start()` sequence.
        let farm = load_gltf_scene(asset_path("models/farm"))?;
        let horse = load_gltf_scene(asset_path("models/horse"))?;
        visuals.set_templates(extract_farm_prop_templates(&farm)?, horse);
        let windmill = create_windmill(&farm).ok();
        mount_farm(&farm, &scene_graph.root);
        let ground = sample_farm_terrain(&farm).map(Into::into);
        let mut game = game;
        game.mark_ready(ground);
        camera_rig.update(
            &mut scene_graph.camera,
            1.0,
            game.displayed_height() as f32,
            game.objects_dropped(),
        );
        indicator.hide();

        Ok(Self {
            template,
            display: GlDisplayCreationState::Builder(Box::new(display_builder)),
            renderer: None,
            game_ui: None,
            surface: None,
            context: None,
            exit_error: None,
            audio,
            scene_graph,
            particles,
            camera_rig,
            visuals,
            indicator,
            game,
            windmill,
            storage,
            credits_open: false,
            pointer_x: -1.0,
            pointer_y: -1.0,
            pointer_down: false,
            backdrop_focus: 0.0,
            clock: Instant::now(),
            previous_time: 0.0,
            is_viewer_visible: true,
            render_requested: true,
            input_bounds: InputBounds {
                width: 1.0,
                height: 1.0,
            },
            aim_point: PlayPlanePoint::default(),
            pixel_ratio: 1.0,
            status_copy: "Stable enough".to_owned(),
            edge_probe_requested: false,
            forced_screen: None,
        })
    }

    fn now_ms(&self) -> f64 {
        self.clock.elapsed().as_secs_f64() * 1_000.0
    }

    fn with_game_deps<R>(&mut self, call: impl FnOnce(&mut Game, &mut GameDeps<'_>) -> R) -> R {
        let Self {
            game,
            audio,
            camera_rig,
            indicator,
            particles,
            scene_graph,
            visuals,
            storage,
            status_copy,
            ..
        } = self;
        let mut announce = |text: &str| *status_copy = text.to_owned();
        let mut deps = GameDeps {
            audio,
            camera_rig,
            indicator,
            particles,
            scene_graph,
            visuals,
            storage,
            announce: &mut announce,
        };
        call(game, &mut deps)
    }

    fn start_game(&mut self, mode: GameMode, started_from: Option<f64>) {
        if !self.game.is_ready() || self.game.phase() == GamePhase::Loading {
            return;
        }
        self.credits_open = false;
        let now = self.now_ms();
        self.previous_time = now;
        self.with_game_deps(|game, deps| game.start_round(deps, mode, now, started_from));
        self.render_requested = true;
    }

    fn return_to_menu(&mut self) {
        self.credits_open = false;
        self.with_game_deps(|game, deps| game.leave_round(deps));
        self.render_requested = true;
    }

    /// Native counterpart of `aimFromClient`: unproject the cursor into the physics plane.
    fn aim_from_client(&mut self, client_x: f32, client_y: f32, now: f64) {
        let aspect = match &self.scene_graph.camera.projection {
            Projection::Perspective(projection) => projection.read().unwrap().aspect,
            _ => 1.0,
        };
        let ndc_x = client_x / self.input_bounds.width * 2.0 - 1.0;
        let ndc_y = 1.0 - client_y / self.input_bounds.height * 2.0;
        if !unproject_to_play_plane(
            &mut self.aim_point,
            &self.scene_graph.camera,
            ndc_x,
            ndc_y,
            aspect,
        ) {
            return;
        }
        let point = self.aim_point;
        self.with_game_deps(|game, deps| {
            game.aim_at(deps, f64::from(point.x), f64::from(point.y), now)
        });
    }

    fn track_pointer(&mut self, position: PhysicalPosition<f64>) {
        self.pointer_x = position.x as f32 / self.pixel_ratio;
        self.pointer_y = position.y as f32 / self.pixel_ratio;
    }

    fn handle_ui_press(&mut self, event_at: f64) -> bool {
        let button = self.game_ui.as_ref().and_then(|ui| {
            ui.buttons.iter().copied().find(|button| {
                self.pointer_x >= button.x
                    && self.pointer_y >= button.y
                    && self.pointer_x <= button.x + button.width
                    && self.pointer_y <= button.y + button.height
            })
        });
        let Some(button) = button else {
            return false;
        };
        let screen = self.get_ui_screen();
        match button.id {
            UiButtonId::Credits => self.credits_open = !self.credits_open,
            UiButtonId::Fullscreen => self.toggle_fullscreen(),
            UiButtonId::Mute => self.audio.set_muted(!self.audio.muted()),
            UiButtonId::Time if screen == UiScreen::Title => {
                self.start_game(GameMode::Time, Some(event_at))
            }
            UiButtonId::Steady if screen == UiScreen::Title => {
                self.start_game(GameMode::Steady, Some(event_at))
            }
            UiButtonId::Again if screen == UiScreen::Result => {
                self.start_game(self.game.mode(), Some(event_at))
            }
            UiButtonId::Menu => self.return_to_menu(),
            _ => return false,
        }
        self.render_requested = true;
        true
    }

    fn toggle_fullscreen(&mut self) {
        let Some(surface) = &self.surface else {
            return;
        };
        let fullscreen = if surface.window.fullscreen().is_some() {
            None
        } else {
            Some(Fullscreen::Borderless(None))
        };
        surface.window.set_fullscreen(fullscreen);
        self.render_requested = true;
    }

    fn resize_canvas(&mut self, size: PhysicalSize<u32>) -> Result<(), String> {
        let width = size.width.max(1);
        let height = size.height.max(1);
        self.pixel_ratio = self
            .surface
            .as_ref()
            .map(|surface| surface.window.scale_factor().min(2.0) as f32)
            .unwrap_or(1.0);
        self.input_bounds.width = width as f32 / self.pixel_ratio;
        self.input_bounds.height = height as f32 / self.pixel_ratio;
        if let Some(renderer) = self.renderer.as_mut() {
            let resized = renderer.resize(width, height)?;
            if resized {
                if let Some(ui) = self.game_ui.as_mut() {
                    ui.resize(
                        self.input_bounds.width,
                        self.input_bounds.height,
                        self.pixel_ratio,
                    )?;
                }
                set_camera_aspect(&mut self.scene_graph, width, height);
                self.render_requested = true;
            }
        }
        Ok(())
    }

    fn get_ui_screen(&self) -> UiScreen {
        if let Some(screen) = self.forced_screen {
            return screen;
        }
        match self.game.phase() {
            GamePhase::Loading => UiScreen::Loading,
            GamePhase::Playing => UiScreen::Playing,
            GamePhase::Settling => UiScreen::TimeUp,
            GamePhase::Finished => UiScreen::Result,
            GamePhase::Ready => UiScreen::Title,
        }
    }

    fn enter_frame(&mut self) {
        let now = self.now_ms();
        let delta_time = ((now - self.previous_time) / 1_000.0).clamp(0.0, 0.05);
        self.previous_time = now;
        if self.game.phase() == GamePhase::Loading || !self.is_viewer_visible {
            return;
        }

        let game_is_moving = self.game.is_running();
        self.with_game_deps(|game, deps| game.update(deps, now, delta_time));
        if game_is_moving {
            self.render_requested = true;
        }
        if self
            .windmill
            .as_mut()
            .is_some_and(|windmill| windmill.update(delta_time as f32))
        {
            self.render_requested = true;
        }
        self.audio.update(now, self.game.is_running());

        let screen = self.get_ui_screen();
        let wants_backdrop = if matches!(screen, UiScreen::Title | UiScreen::Result) {
            1.0
        } else {
            0.0
        };
        let focused = self.backdrop_focus
            + (wants_backdrop - self.backdrop_focus) * (1.0 - (-delta_time * 5.5).exp());
        if (focused - self.backdrop_focus).abs() > 0.0005 {
            self.backdrop_focus = focused;
            self.render_requested = true;
        } else if self.backdrop_focus != wants_backdrop {
            self.backdrop_focus = wants_backdrop;
            self.render_requested = true;
        }

        let particles_are_moving = self.particles.step(delta_time as f32);
        if particles_are_moving {
            self.render_requested = true;
        }
        if self.camera_rig.update(
            &mut self.scene_graph.camera,
            delta_time as f32,
            self.game.displayed_height() as f32,
            self.game.objects_dropped(),
        ) {
            self.render_requested = true;
        }
        let camera_is_moving = self.camera_rig.is_moving();
        if self.render_requested || game_is_moving || particles_are_moving || camera_is_moving {
            if let Some(surface) = &self.surface {
                surface.window.request_redraw();
            }
        }
    }

    fn render_frame(&mut self) -> Result<(), String> {
        let renderer = self
            .renderer
            .as_mut()
            .ok_or_else(|| "render requested before the Flight renderer was ready".to_owned())?;
        renderer.draw_scene(&self.scene_graph);

        let count_progress = self.game.count_progress();
        let final_meters = get_stack_height_meters(self.game.final_height());
        let shown_meters = final_meters * ease_out_cubic(count_progress);
        let model = UiModel {
            best_text: self
                .game
                .record_before_round()
                .filter(|_| !self.game.beat_the_record())
                .map(|meters| format!("BEST {}", format_meters(meters)))
                .unwrap_or_default(),
            count_progress,
            credits_open: self.credits_open,
            is_record: self.game.beat_the_record(),
            hands_shown: self.game.result_hands_shown(),
            now: self.now_ms(),
            pointer_down: self.pointer_down,
            pointer_x: self.pointer_x,
            pointer_y: self.pointer_y,
            height_text: format_meters(if count_progress >= 1.0 {
                final_meters
            } else {
                shown_meters
            }),
            live_height_text: format_meters(get_stack_height_meters(self.game.displayed_height())),
            next_piece_glyph: piece_glyph(self.game.next_kind()).to_owned(),
            mode: self.game.mode(),
            muted: self.audio.muted(),
            screen: self.get_ui_screen(),
            seconds_left: self.game.seconds_left(),
            time_up_progress: if self.forced_screen == Some(UiScreen::TimeUp) {
                1.0
            } else {
                self.game.time_up_progress()
            },
            credits_text: None,
        };
        if let Some(ui) = self.game_ui.as_mut() {
            if ui.update(&model) {
                self.render_requested = true;
            }
            ui.render()?;
        }
        self.edge_probe_requested = false;
        self.render_requested = false;
        Ok(())
    }

    fn update_window_title(&self) {
        let Some(surface) = &self.surface else {
            return;
        };
        let title = match self.game.phase() {
            GamePhase::Loading => format!("{WINDOW_TITLE} — loading"),
            GamePhase::Ready => format!("{WINDOW_TITLE} — Time Challenge · Steady Hands"),
            GamePhase::Playing => format!(
                "{WINDOW_TITLE} — {} · {} · {} placed · next {}",
                mode_label(self.game.mode()),
                format_meters(get_stack_height_meters(self.game.displayed_height())),
                self.game.objects_dropped(),
                piece_glyph(self.game.next_kind()),
            ),
            GamePhase::Settling => format!("{WINDOW_TITLE} — settling the pile…"),
            GamePhase::Finished => format!(
                "{WINDOW_TITLE} — {} · play again or menu",
                format_meters(get_stack_height_meters(self.game.final_height())),
            ),
        };
        surface.window.set_title(&title);
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl Into<String>) {
        let error = error.into();
        self.audio.stop_all();
        eprintln!("Horse Stacker failed: {error}");
        self.status_copy = "Game unavailable".to_owned();
        self.exit_error = Some(error);
        event_loop.exit();
    }

    fn redraw(&mut self) -> Result<(), String> {
        self.render_frame()?;
        let surface = self
            .surface
            .as_ref()
            .ok_or_else(|| "redraw requested without a window surface".to_owned())?;
        let context = self
            .context
            .as_ref()
            .ok_or_else(|| "redraw requested without a GL context".to_owned())?;
        surface.window.pre_present_notify();
        surface
            .gl_surface
            .swap_buffers(context)
            .map_err(|error| format!("swap buffers: {error}"))
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, logical_key: Key<&str>, event_at: f64) {
        if logical_key == Key::Named(NamedKey::Escape) {
            if self.game.mode() == GameMode::Steady && self.game.is_running() {
                self.return_to_menu();
            } else {
                event_loop.exit();
            }
            return;
        }
        if self.game.phase() == GamePhase::Ready {
            match logical_key {
                Key::Named(NamedKey::Enter) => self.start_game(GameMode::Time, Some(event_at)),
                Key::Character(value) if value.eq_ignore_ascii_case("s") => {
                    self.start_game(GameMode::Steady, Some(event_at))
                }
                _ => {}
            }
            return;
        }
        if self.game.phase() == GamePhase::Finished && logical_key == Key::Named(NamedKey::Enter) {
            self.start_game(self.game.mode(), Some(event_at));
            return;
        }
        if self.game.phase() != GamePhase::Playing {
            return;
        }
        match logical_key {
            Key::Named(NamedKey::ArrowLeft) => self
                .with_game_deps(|game, deps| game.nudge_aim(deps, -KEYBOARD_NUDGE, 0.0, event_at)),
            Key::Named(NamedKey::ArrowRight) => self
                .with_game_deps(|game, deps| game.nudge_aim(deps, KEYBOARD_NUDGE, 0.0, event_at)),
            Key::Named(NamedKey::ArrowUp) => self
                .with_game_deps(|game, deps| game.nudge_aim(deps, 0.0, KEYBOARD_NUDGE, event_at)),
            Key::Named(NamedKey::ArrowDown) => self
                .with_game_deps(|game, deps| game.nudge_aim(deps, 0.0, -KEYBOARD_NUDGE, event_at)),
            Key::Named(NamedKey::Enter) | Key::Character(" ") => {
                self.with_game_deps(|game, deps| game.place(deps, event_at, Some(event_at)));
            }
            Key::Named(NamedKey::Backspace) => self.return_to_menu(),
            _ => return,
        }
        self.render_requested = true;
    }
}

impl ApplicationHandler for HorseStackerApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.is_viewer_visible = true;
        self.previous_time = self.now_ms();
        let (window, gl_config) = match &self.display {
            GlDisplayCreationState::Builder(display_builder) => {
                let built = display_builder.as_ref().clone().build(
                    event_loop,
                    self.template.clone(),
                    pick_gl_config,
                );
                let (window, gl_config) = match built {
                    Ok((Some(window), config)) => (window, config),
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
                let gl_config = self.context.as_ref().expect("context retained").config();
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
            .expect("context created before surface");
        if let Err(error) = context.make_current(&gl_surface) {
            self.fail(event_loop, format!("make GL context current: {error}"));
            return;
        }
        let _ =
            gl_surface.set_swap_interval(context, SwapInterval::Wait(NonZeroU32::new(1).unwrap()));
        let size = window.inner_size();
        self.surface = Some(AppSurface { gl_surface, window });
        if self.renderer.is_none() {
            match create_native_scene_renderer(
                &gl_config.display(),
                size.width.max(1),
                size.height.max(1),
            ) {
                Ok(renderer) => {
                    self.game_ui = Some(create_game_ui_2d(&renderer.render_state, 1.0));
                    self.renderer = Some(renderer);
                }
                Err(error) => {
                    self.fail(event_loop, error);
                    return;
                }
            }
        }
        if let Err(error) = self.resize_canvas(size) {
            self.fail(event_loop, error);
            return;
        }
        self.render_requested = true;
        if let Some(surface) = &self.surface {
            surface.window.request_redraw();
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.is_viewer_visible = false;
        self.game.reset_step_accumulator();
        self.surface = None;
        if let Some(context) = self.context.take() {
            self.context = Some(
                context
                    .make_not_current()
                    .expect("suspend current GL context")
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
            WindowEvent::CursorMoved { position, .. } => {
                self.track_pointer(position);
                self.render_requested = true;
                if self.game.phase() == GamePhase::Playing {
                    self.aim_from_client(self.pointer_x, self.pointer_y, self.now_ms());
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer_x = -1.0;
                self.pointer_y = -1.0;
                self.pointer_down = false;
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.pointer_down = state == ElementState::Pressed;
                self.render_requested = true;
                if state == ElementState::Pressed {
                    let now = self.now_ms();
                    if !self.handle_ui_press(now) && self.game.phase() == GamePhase::Playing {
                        self.aim_from_client(self.pointer_x, self.pointer_y, now);
                        self.with_game_deps(|game, deps| game.place(deps, now, Some(now)));
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => self.handle_key(event_loop, logical_key.as_ref(), self.now_ms()),
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                if let (Some(surface), Some(context)) = (&self.surface, &self.context) {
                    surface.gl_surface.resize(
                        context,
                        NonZeroU32::new(size.width).unwrap(),
                        NonZeroU32::new(size.height).unwrap(),
                    );
                }
                if let Err(error) = self.resize_canvas(size) {
                    self.fail(event_loop, error);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(surface) = &self.surface {
                    let size = surface.window.inner_size();
                    if let Err(error) = self.resize_canvas(size) {
                        self.fail(event_loop, error);
                    }
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
        self.enter_frame();
        self.update_window_title();
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.audio.stop_all();
        self.game_ui = None;
        self.renderer = None;
        self.surface = None;
        self.context = None;
    }
}

struct AppSurface {
    gl_surface: Surface<WindowSurface>,
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

fn piece_glyph(kind: StackObjectKind) -> &'static str {
    match kind {
        StackObjectKind::Chickens => "🐔",
        StackObjectKind::Cow => "🐄",
        StackObjectKind::Hay => "🌾",
        StackObjectKind::Horse => "🐴",
    }
}

fn mode_label(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Time => "Time Challenge",
        GameMode::Steady => "Steady Hands",
    }
}

fn ease_out_cubic(value: f64) -> f64 {
    1.0 - (1.0 - value).powi(3)
}
fn format_meters(meters: f64) -> String {
    format!("{meters:.2} m")
}

fn create_gl_context(window: &Window, gl_config: &Config) -> Result<NotCurrentContext, String> {
    let raw_window_handle = window.window_handle().ok().map(|handle| handle.as_raw());
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
