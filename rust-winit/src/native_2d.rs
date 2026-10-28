//! Native Canvas 2D and image capabilities for Flight's text renderers.
//!
//! The browser supplies these capabilities to the TypeScript app through `host-web`. Flight keeps
//! them host-defined on Rust, so the winit shell supplies a small software text surface and exposes
//! each finished surface as the RGBA token consumed by `flight-render-gl`.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock, Weak};

use ab_glyph::{Font, FontArc, PxScale, ScaleFont, point};
use flight_sdk::{
    Canvas2DContext, CanvasRenderingContext2D, CanvasSurface, CanvasTextMetrics,
    GlImageSourceToken, HostCanvasCapability, HostImageCapability, HostImageDimensions,
    HostImageSource, ImageResolverState, ImageResource, ProviderRef, Ref, SurfaceRef,
    allocate_surface, create_image_resolver_state, create_image_resource, get_entity_id,
};

#[derive(Clone)]
pub struct Native2DHost {
    pub canvas: HostCanvasCapability,
    pub image: HostImageCapability,
    pub image_resolver: Ref<ImageResolverState>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Rect {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

#[derive(Clone, Copy, Debug)]
struct FontSpec {
    bold: bool,
    serif: bool,
    size: f32,
}

impl Default for FontSpec {
    fn default() -> Self {
        Self {
            bold: false,
            serif: false,
            size: 12.0,
        }
    }
}

#[derive(Clone, Debug)]
enum PathCommand {
    Move(f32, f32),
    Line(f32, f32),
    Rectangle(f32, f32, f32, f32),
}

#[derive(Clone)]
struct CanvasSnapshot {
    clip: Option<Rect>,
    fill: [u8; 4],
    font: FontSpec,
    global_alpha: f32,
    line_width: f32,
    stroke: [u8; 4],
    text_align: TextAlign,
    transform: [f32; 6],
}

#[derive(Clone, Copy, Default)]
enum TextAlign {
    Center,
    End,
    #[default]
    Start,
}

struct CanvasState {
    clip: Option<Rect>,
    fill: [u8; 4],
    font: FontSpec,
    global_alpha: f32,
    line_width: f32,
    path: Vec<PathCommand>,
    pixels: Vec<u8>,
    saved: Vec<CanvasSnapshot>,
    stroke: [u8; 4],
    text_align: TextAlign,
    transform: [f32; 6],
}

impl CanvasState {
    fn new(width: u32, height: u32) -> Self {
        Self {
            clip: None,
            fill: [0, 0, 0, 255],
            font: FontSpec::default(),
            global_alpha: 1.0,
            line_width: 1.0,
            path: Vec::new(),
            pixels: vec![0; width as usize * height as usize * 4],
            saved: Vec::new(),
            stroke: [0, 0, 0, 255],
            text_align: TextAlign::Start,
            transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        }
    }

    fn snapshot(&self) -> CanvasSnapshot {
        CanvasSnapshot {
            clip: self.clip,
            fill: self.fill,
            font: self.font,
            global_alpha: self.global_alpha,
            line_width: self.line_width,
            stroke: self.stroke,
            text_align: self.text_align,
            transform: self.transform,
        }
    }

    fn restore(&mut self, value: CanvasSnapshot) {
        self.clip = value.clip;
        self.fill = value.fill;
        self.font = value.font;
        self.global_alpha = value.global_alpha;
        self.line_width = value.line_width;
        self.stroke = value.stroke;
        self.text_align = value.text_align;
        self.transform = value.transform;
    }
}

#[derive(Default)]
struct NativeFonts {
    sans: Option<FontArc>,
    sans_bold: Option<FontArc>,
    serif: Option<FontArc>,
    serif_bold: Option<FontArc>,
}

impl NativeFonts {
    fn load() -> Self {
        let sans = load_font("HORSE_STACKER_SANS_FONT", SANS_REGULAR);
        let sans_bold =
            load_font("HORSE_STACKER_SANS_BOLD_FONT", SANS_BOLD).or_else(|| sans.clone());
        let serif = load_font("HORSE_STACKER_SERIF_FONT", SERIF_REGULAR).or_else(|| sans.clone());
        let serif_bold = load_font("HORSE_STACKER_SERIF_BOLD_FONT", SERIF_BOLD)
            .or_else(|| serif.clone())
            .or_else(|| sans_bold.clone());
        eprintln!(
            "[horse-stacker] text host: sans={} serif={}",
            sans.is_some(),
            serif.is_some()
        );
        Self {
            sans,
            sans_bold,
            serif,
            serif_bold,
        }
    }

    fn select(&self, spec: FontSpec) -> Option<&FontArc> {
        match (spec.serif, spec.bold) {
            (true, true) => self.serif_bold.as_ref(),
            (true, false) => self.serif.as_ref(),
            (false, true) => self.sans_bold.as_ref(),
            (false, false) => self.sans.as_ref(),
        }
    }
}

struct NativeCanvas2D {
    fonts: Arc<NativeFonts>,
    height: u32,
    image: Mutex<Option<Weak<RwLock<ImageResource>>>>,
    provider: ProviderRef,
    state: Mutex<CanvasState>,
    width: u32,
}

impl NativeCanvas2D {
    fn new(width: u32, height: u32, fonts: Arc<NativeFonts>, provider: ProviderRef) -> Self {
        Self {
            fonts,
            height,
            image: Mutex::new(None),
            provider,
            state: Mutex::new(CanvasState::new(width, height)),
            width,
        }
    }

    fn source(&self) -> Arc<HostImageSource> {
        let pixels = self.state.lock().unwrap().pixels.clone();
        Arc::new(HostImageSource {
            provider: self.provider.clone(),
            handle: Arc::new(GlImageSourceToken {
                width: self.width,
                height: self.height,
                ready: true,
                pixels: Arc::new(pixels),
            }),
        })
    }

    fn attach_image(&self, image: &Ref<ImageResource>) {
        *self.image.lock().unwrap() = Some(Arc::downgrade(image));
    }

    fn refresh_image(&self) {
        let image = self.image.lock().unwrap().as_ref().and_then(Weak::upgrade);
        if let Some(image) = image {
            image.write().unwrap().source = self.source();
        }
    }

    fn logical_text_width(&self, text: &str, spec: FontSpec) -> f32 {
        text_width(self.fonts.select(spec), text, spec.size)
    }

    fn mutate_pixels(&self, draw: impl FnOnce(&mut CanvasState, u32, u32, &NativeFonts)) {
        {
            let mut state = self.state.lock().unwrap();
            draw(&mut state, self.width, self.height, &self.fonts);
        }
        self.refresh_image();
    }
}

impl Canvas2DContext for NativeCanvas2D {
    fn begin_path(&self) {
        self.state.lock().unwrap().path.clear();
    }

    fn clear_rect(&self, x: f32, y: f32, width: f32, height: f32) {
        self.mutate_pixels(|state, canvas_width, canvas_height, _| {
            let rect = transformed_rect(state.transform, x, y, width, height);
            for_each_pixel(rect, state.clip, canvas_width, canvas_height, |index| {
                state.pixels[index..index + 4].fill(0);
            });
        });
    }

    fn clip(&self) {
        let mut state = self.state.lock().unwrap();
        let rectangle = state.path.iter().rev().find_map(|command| match command {
            PathCommand::Rectangle(x, y, width, height) => {
                Some(transformed_rect(state.transform, *x, *y, *width, *height))
            }
            _ => None,
        });
        if let Some(rectangle) = rectangle {
            state.clip = Some(match state.clip {
                Some(current) => intersect_rect(current, rectangle),
                None => rectangle,
            });
        }
    }

    fn fill_rect(&self, x: f32, y: f32, width: f32, height: f32) {
        self.mutate_pixels(|state, canvas_width, canvas_height, _| {
            let rectangle = transformed_rect(state.transform, x, y, width, height);
            let color = state.fill;
            let alpha = state.global_alpha;
            for_each_pixel(
                rectangle,
                state.clip,
                canvas_width,
                canvas_height,
                |index| blend_pixel(&mut state.pixels, index, color, alpha),
            );
        });
    }

    fn fill_text(&self, text: &str, x: f32, y: f32) {
        self.mutate_pixels(|state, width, height, fonts| {
            let spec = state.font;
            draw_text(fonts.select(spec), state, width, height, text, x, y);
        });
    }

    fn line_to(&self, x: f32, y: f32) {
        self.state
            .lock()
            .unwrap()
            .path
            .push(PathCommand::Line(x, y));
    }

    fn measure_text(&self, text: &str) -> CanvasTextMetrics {
        let state = self.state.lock().unwrap();
        CanvasTextMetrics {
            width: self.logical_text_width(text, state.font),
        }
    }

    fn move_to(&self, x: f32, y: f32) {
        self.state
            .lock()
            .unwrap()
            .path
            .push(PathCommand::Move(x, y));
    }

    fn rect(&self, x: f32, y: f32, width: f32, height: f32) {
        self.state
            .lock()
            .unwrap()
            .path
            .push(PathCommand::Rectangle(x, y, width, height));
    }

    fn restore(&self) {
        let mut state = self.state.lock().unwrap();
        if let Some(saved) = state.saved.pop() {
            state.restore(saved);
        }
    }

    fn save(&self) {
        let mut state = self.state.lock().unwrap();
        let saved = state.snapshot();
        state.saved.push(saved);
    }

    fn set_fill_style(&self, style: &str) {
        if let Some(color) = parse_color(style) {
            self.state.lock().unwrap().fill = color;
        }
    }

    fn set_font(&self, font: &str) {
        self.state.lock().unwrap().font = parse_font(font);
    }

    fn set_global_alpha(&self, alpha: f32) {
        self.state.lock().unwrap().global_alpha = alpha.clamp(0.0, 1.0);
    }

    fn set_line_width(&self, width: f32) {
        self.state.lock().unwrap().line_width = width.max(0.0);
    }

    fn set_stroke_style(&self, style: &str) {
        if let Some(color) = parse_color(style) {
            self.state.lock().unwrap().stroke = color;
        }
    }

    fn set_text_align(&self, align: &str) {
        self.state.lock().unwrap().text_align = match align {
            "center" => TextAlign::Center,
            "end" | "right" => TextAlign::End,
            _ => TextAlign::Start,
        };
    }

    fn set_text_baseline(&self, _baseline: &str) {}

    fn set_transform(&self, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) {
        self.state.lock().unwrap().transform = [a, b, c, d, e, f];
    }

    fn stroke(&self) {
        self.mutate_pixels(|state, width, height, _| {
            let mut cursor = None;
            let commands = state.path.clone();
            for command in commands {
                match command {
                    PathCommand::Move(x, y) => cursor = Some((x, y)),
                    PathCommand::Line(x, y) => {
                        if let Some((from_x, from_y)) = cursor {
                            stroke_line(state, width, height, from_x, from_y, x, y);
                        }
                        cursor = Some((x, y));
                    }
                    PathCommand::Rectangle(x, y, rect_width, rect_height) => {
                        stroke_rectangle(state, width, height, x, y, rect_width, rect_height);
                    }
                }
            }
        });
    }

    fn stroke_rect(&self, x: f32, y: f32, width: f32, height: f32) {
        self.mutate_pixels(|state, canvas_width, canvas_height, _| {
            stroke_rectangle(state, canvas_width, canvas_height, x, y, width, height);
        });
    }
}

pub fn create_native_2d_host() -> Native2DHost {
    let provider: ProviderRef = Arc::new(());
    let fonts = Arc::new(NativeFonts::load());
    let canvases = Arc::new(Mutex::new(HashMap::<u64, Arc<NativeCanvas2D>>::new()));

    let mut resolver = create_image_resolver_state();
    resolver.register_dimension_resolver(Arc::new(
        |source, dimensions: &mut HostImageDimensions| {
            let Some(token) = source.handle.as_ref().downcast_ref::<GlImageSourceToken>() else {
                return false;
            };
            dimensions.width = token.width;
            dimensions.height = token.height;
            true
        },
    ));
    let image_resolver = Arc::new(RwLock::new(resolver));

    let create_canvases = Arc::clone(&canvases);
    let create_fonts = Arc::clone(&fonts);
    let create_provider = provider.clone();
    let destroy_canvases = Arc::clone(&canvases);
    let canvas = HostCanvasCapability {
        provider: provider.clone(),
        acquire: Arc::new(|_, _| None),
        create: Arc::new(|_, _, _, _| None),
        create_surface: Arc::new(move |width, height| {
            let width = width.ceil().max(1.0) as u32;
            let height = height.ceil().max(1.0) as u32;
            let surface: Ref<CanvasSurface> = allocate_surface(None);
            let native = Arc::new(NativeCanvas2D::new(
                width,
                height,
                Arc::clone(&create_fonts),
                create_provider.clone(),
            ));
            surface.write().unwrap().context = CanvasRenderingContext2D {
                provider: create_provider.clone(),
                handle: native.clone(),
            };
            let id = get_entity_id(&*surface.read().unwrap());
            create_canvases.lock().unwrap().insert(id, native);
            Some(surface)
        }),
        destroy_surface: Arc::new(move |surface| {
            let id = get_entity_id(&*surface.read().unwrap());
            destroy_canvases.lock().unwrap().remove(&id);
        }),
        release: Arc::new(|_| {}),
    };

    let image_canvases = Arc::clone(&canvases);
    let image_provider = provider.clone();
    let resource_resolver = Arc::clone(&image_resolver);
    let image = HostImageCapability {
        create_image_from_bitmap: None,
        create_image_from_surface: Some(Arc::new(move |surface: &SurfaceRef| {
            let id = get_entity_id(&*surface.read().unwrap());
            let canvas = image_canvases
                .lock()
                .unwrap()
                .get(&id)
                .cloned()
                .expect("native image host received a foreign canvas surface");
            let resource =
                create_image_resource(canvas.source(), &resource_resolver.read().unwrap());
            canvas.attach_image(&resource);
            resource
        })),
        load_image_from_url: Arc::new(|_, _, _| Box::pin(std::future::pending())),
        provider: image_provider,
    };

    Native2DHost {
        canvas,
        image,
        image_resolver,
    }
}

fn draw_text(
    font: Option<&FontArc>,
    state: &mut CanvasState,
    width: u32,
    height: u32,
    text: &str,
    x: f32,
    y: f32,
) {
    let [a, b, c, d, e, f] = state.transform;
    let scale_factor = ((a.abs() + d.abs()) * 0.5).max(0.01);
    let scale = font.map(|font| css_pixel_scale(font, state.font.size * scale_factor));
    let measured = text_width(font, text, state.font.size) * scale_factor;
    let aligned_x = match state.text_align {
        TextAlign::Center => x - measured / (2.0 * scale_factor),
        TextAlign::End => x - measured / scale_factor,
        TextAlign::Start => x,
    };
    let (mut cursor_x, baseline_y) = transform_point([a, b, c, d, e, f], aligned_x, y);
    let mut previous = None;
    let color = state.fill;
    let global_alpha = state.global_alpha;
    let clip = state.clip;
    for character in text.chars() {
        if is_game_emoji(character) {
            draw_game_emoji(
                state,
                width,
                height,
                character,
                cursor_x,
                baseline_y,
                state.font.size * scale_factor,
            );
            cursor_x += state.font.size * scale_factor;
            previous = None;
            continue;
        }
        let (Some(font), Some(scale)) = (font, scale) else {
            cursor_x += state.font.size * 0.56 * scale_factor;
            previous = None;
            continue;
        };
        let scaled = font.as_scaled(scale);
        let id = scaled.glyph_id(character);
        if let Some(previous) = previous {
            cursor_x += scaled.kern(previous, id);
        }
        let glyph = id.with_scale_and_position(scale, point(cursor_x, baseline_y));
        if let Some(outline) = font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            outline.draw(|draw_x, draw_y, coverage| {
                let px = bounds.min.x.floor() as i32 + draw_x as i32;
                let py = bounds.min.y.floor() as i32 + draw_y as i32;
                if let Some(index) = pixel_index(px, py, clip, width, height) {
                    blend_pixel(&mut state.pixels, index, color, global_alpha * coverage);
                }
            });
        }
        cursor_x += scaled.h_advance(id);
        previous = Some(id);
    }
}

fn css_pixel_scale(font: &FontArc, pixels_per_em: f32) -> PxScale {
    let height = font.height_unscaled();
    let units_per_em = font.units_per_em().unwrap_or(height);
    PxScale::from(css_pixels_to_ab_glyph_height(
        pixels_per_em,
        height,
        units_per_em,
    ))
}

fn css_pixels_to_ab_glyph_height(
    pixels_per_em: f32,
    unscaled_height: f32,
    units_per_em: f32,
) -> f32 {
    pixels_per_em.max(0.01) * unscaled_height / units_per_em
}

fn text_width(font: Option<&FontArc>, text: &str, size: f32) -> f32 {
    let scaled = font.map(|font| font.as_scaled(css_pixel_scale(font, size)));
    let mut width = 0.0;
    let mut previous = None;
    for character in text.chars() {
        if is_game_emoji(character) {
            width += size;
            previous = None;
            continue;
        }
        let Some(scaled) = scaled.as_ref() else {
            width += size * 0.56;
            previous = None;
            continue;
        };
        let id = scaled.glyph_id(character);
        if let Some(previous) = previous {
            width += scaled.kern(previous, id);
        }
        width += scaled.h_advance(id);
        previous = Some(id);
    }
    width
}

fn is_game_emoji(character: char) -> bool {
    matches!(character, '🐔' | '🐄' | '🌾' | '🐴')
}

fn draw_game_emoji(
    state: &mut CanvasState,
    width: u32,
    height: u32,
    character: char,
    cursor_x: f32,
    baseline_y: f32,
    em: f32,
) {
    let glyph_size = em * 0.94;
    let left = cursor_x + (em - glyph_size) * 0.5;
    let top = baseline_y - em * 0.86;
    let x0 = left.floor() as i32;
    let y0 = top.floor() as i32;
    let x1 = (left + glyph_size).ceil() as i32;
    let y1 = (top + glyph_size).ceil() as i32;
    for pixel_y in y0..y1 {
        for pixel_x in x0..x1 {
            let u = (pixel_x as f32 + 0.5 - left) / glyph_size;
            let v = (pixel_y as f32 + 0.5 - top) / glyph_size;
            let Some(color) = game_emoji_pixel(character, u, v) else {
                continue;
            };
            if let Some(index) = pixel_index(pixel_x, pixel_y, state.clip, width, height) {
                blend_pixel(&mut state.pixels, index, color, state.global_alpha);
            }
        }
    }
}

fn game_emoji_pixel(character: char, u: f32, v: f32) -> Option<[u8; 4]> {
    fn ellipse(u: f32, v: f32, cx: f32, cy: f32, rx: f32, ry: f32) -> bool {
        ((u - cx) / rx).powi(2) + ((v - cy) / ry).powi(2) <= 1.0
    }

    match character {
        '🐴' => {
            let mut color = None;
            if ellipse(u, v, 0.28, 0.2, 0.14, 0.22) || ellipse(u, v, 0.72, 0.2, 0.14, 0.22) {
                color = Some([151, 82, 42, 255]);
            }
            if ellipse(u, v, 0.5, 0.5, 0.34, 0.45) {
                color = Some([222, 139, 76, 255]);
            }
            if ellipse(u, v, 0.5, 0.72, 0.25, 0.18) {
                color = Some([247, 181, 112, 255]);
            }
            if ellipse(u, v, 0.38, 0.43, 0.045, 0.055)
                || ellipse(u, v, 0.62, 0.43, 0.045, 0.055)
                || ellipse(u, v, 0.43, 0.73, 0.032, 0.025)
                || ellipse(u, v, 0.57, 0.73, 0.032, 0.025)
            {
                color = Some([49, 31, 25, 255]);
            }
            color
        }
        '🐄' => {
            let mut color = None;
            if ellipse(u, v, 0.24, 0.3, 0.19, 0.11) || ellipse(u, v, 0.76, 0.3, 0.19, 0.11) {
                color = Some([176, 133, 91, 255]);
            }
            if ellipse(u, v, 0.5, 0.52, 0.36, 0.4) {
                color = Some([244, 238, 221, 255]);
            }
            if ellipse(u, v, 0.37, 0.42, 0.13, 0.18) || ellipse(u, v, 0.67, 0.58, 0.12, 0.16) {
                color = Some([63, 55, 49, 255]);
            }
            if ellipse(u, v, 0.5, 0.73, 0.25, 0.16) {
                color = Some([232, 159, 163, 255]);
            }
            if ellipse(u, v, 0.4, 0.5, 0.035, 0.045)
                || ellipse(u, v, 0.6, 0.5, 0.035, 0.045)
                || ellipse(u, v, 0.43, 0.74, 0.03, 0.025)
                || ellipse(u, v, 0.57, 0.74, 0.03, 0.025)
            {
                color = Some([35, 31, 28, 255]);
            }
            color
        }
        '🐔' => {
            let mut color = None;
            if ellipse(u, v, 0.43, 0.16, 0.09, 0.11)
                || ellipse(u, v, 0.56, 0.14, 0.09, 0.11)
                || ellipse(u, v, 0.67, 0.2, 0.08, 0.1)
            {
                color = Some([207, 52, 48, 255]);
            }
            if ellipse(u, v, 0.49, 0.53, 0.36, 0.39) {
                color = Some([249, 240, 213, 255]);
            }
            if u > 0.72 && u < 0.96 && (v - 0.51).abs() < (0.96 - u) * 0.5 {
                color = Some([239, 151, 38, 255]);
            }
            if ellipse(u, v, 0.62, 0.43, 0.045, 0.05) {
                color = Some([35, 31, 28, 255]);
            }
            color
        }
        '🌾' => {
            let stem = (u - (0.66 - v * 0.24)).abs() < 0.035 && v > 0.22;
            let grain = (0..5).any(|index| {
                let y = 0.2 + index as f32 * 0.115;
                ellipse(u, v, 0.53, y, 0.12, 0.065) || ellipse(u, v, 0.72, y + 0.045, 0.12, 0.065)
            });
            (stem || grain).then_some([224, 174, 51, 255])
        }
        _ => None,
    }
}

fn stroke_rectangle(
    state: &mut CanvasState,
    width: u32,
    height: u32,
    x: f32,
    y: f32,
    rect_width: f32,
    rect_height: f32,
) {
    stroke_line(state, width, height, x, y, x + rect_width, y);
    stroke_line(
        state,
        width,
        height,
        x + rect_width,
        y,
        x + rect_width,
        y + rect_height,
    );
    stroke_line(
        state,
        width,
        height,
        x + rect_width,
        y + rect_height,
        x,
        y + rect_height,
    );
    stroke_line(state, width, height, x, y + rect_height, x, y);
}

fn stroke_line(
    state: &mut CanvasState,
    width: u32,
    height: u32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
) {
    let from = transform_point(state.transform, x0, y0);
    let to = transform_point(state.transform, x1, y1);
    let scale = ((state.transform[0].abs() + state.transform[3].abs()) * 0.5).max(0.01);
    let radius = (state.line_width * scale * 0.5).max(0.5);
    let bounds = Rect {
        x0: (from.0.min(to.0) - radius - 1.0).floor() as i32,
        y0: (from.1.min(to.1) - radius - 1.0).floor() as i32,
        x1: (from.0.max(to.0) + radius + 1.0).ceil() as i32,
        y1: (from.1.max(to.1) + radius + 1.0).ceil() as i32,
    };
    for y in bounds.y0..bounds.y1 {
        for x in bounds.x0..bounds.x1 {
            let coverage = (radius + 0.5
                - distance_to_segment(x as f32 + 0.5, y as f32 + 0.5, from, to))
            .clamp(0.0, 1.0);
            if coverage == 0.0 {
                continue;
            }
            if let Some(index) = pixel_index(x, y, state.clip, width, height) {
                blend_pixel(
                    &mut state.pixels,
                    index,
                    state.stroke,
                    state.global_alpha * coverage,
                );
            }
        }
    }
}

fn distance_to_segment(x: f32, y: f32, from: (f32, f32), to: (f32, f32)) -> f32 {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let length_squared = dx * dx + dy * dy;
    if length_squared == 0.0 {
        return ((x - from.0).powi(2) + (y - from.1).powi(2)).sqrt();
    }
    let t = (((x - from.0) * dx + (y - from.1) * dy) / length_squared).clamp(0.0, 1.0);
    let nearest_x = from.0 + t * dx;
    let nearest_y = from.1 + t * dy;
    ((x - nearest_x).powi(2) + (y - nearest_y).powi(2)).sqrt()
}

fn transform_point(transform: [f32; 6], x: f32, y: f32) -> (f32, f32) {
    (
        transform[0] * x + transform[2] * y + transform[4],
        transform[1] * x + transform[3] * y + transform[5],
    )
}

fn transformed_rect(transform: [f32; 6], x: f32, y: f32, width: f32, height: f32) -> Rect {
    let points = [
        transform_point(transform, x, y),
        transform_point(transform, x + width, y),
        transform_point(transform, x, y + height),
        transform_point(transform, x + width, y + height),
    ];
    Rect {
        x0: points
            .iter()
            .map(|point| point.0)
            .fold(f32::INFINITY, f32::min)
            .floor() as i32,
        y0: points
            .iter()
            .map(|point| point.1)
            .fold(f32::INFINITY, f32::min)
            .floor() as i32,
        x1: points
            .iter()
            .map(|point| point.0)
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil() as i32,
        y1: points
            .iter()
            .map(|point| point.1)
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil() as i32,
    }
}

fn intersect_rect(first: Rect, second: Rect) -> Rect {
    Rect {
        x0: first.x0.max(second.x0),
        y0: first.y0.max(second.y0),
        x1: first.x1.min(second.x1),
        y1: first.y1.min(second.y1),
    }
}

fn for_each_pixel(
    rectangle: Rect,
    clip: Option<Rect>,
    width: u32,
    height: u32,
    mut draw: impl FnMut(usize),
) {
    let rectangle = clip.map_or(rectangle, |clip| intersect_rect(rectangle, clip));
    let x0 = rectangle.x0.max(0).min(width as i32);
    let y0 = rectangle.y0.max(0).min(height as i32);
    let x1 = rectangle.x1.max(x0).min(width as i32);
    let y1 = rectangle.y1.max(y0).min(height as i32);
    for y in y0..y1 {
        for x in x0..x1 {
            draw((y as usize * width as usize + x as usize) * 4);
        }
    }
}

fn pixel_index(x: i32, y: i32, clip: Option<Rect>, width: u32, height: u32) -> Option<usize> {
    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
        return None;
    }
    if let Some(clip) = clip
        && (x < clip.x0 || y < clip.y0 || x >= clip.x1 || y >= clip.y1)
    {
        return None;
    }
    Some((y as usize * width as usize + x as usize) * 4)
}

fn blend_pixel(pixels: &mut [u8], index: usize, color: [u8; 4], coverage: f32) {
    let source_alpha = (f32::from(color[3]) / 255.0 * coverage).clamp(0.0, 1.0);
    if source_alpha == 0.0 {
        return;
    }
    let destination_alpha = f32::from(pixels[index + 3]) / 255.0;
    let output_alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    for channel in 0..3 {
        let source = f32::from(color[channel]) / 255.0;
        let destination = f32::from(pixels[index + channel]) / 255.0;
        let output = if output_alpha > 0.0 {
            (source * source_alpha + destination * destination_alpha * (1.0 - source_alpha))
                / output_alpha
        } else {
            0.0
        };
        pixels[index + channel] = (output * 255.0).round() as u8;
    }
    pixels[index + 3] = (output_alpha * 255.0).round() as u8;
}

fn parse_font(value: &str) -> FontSpec {
    let size = value
        .split_whitespace()
        .find_map(|word| word.strip_suffix("px")?.parse::<f32>().ok())
        .unwrap_or(12.0);
    let lowercase = value.to_ascii_lowercase();
    FontSpec {
        bold: lowercase.split_whitespace().any(|word| word == "bold"),
        serif: !lowercase.contains("sans-serif")
            && (lowercase.contains("georgia")
                || lowercase.contains("times")
                || lowercase.ends_with("serif")),
        size,
    }
}

fn parse_color(value: &str) -> Option<[u8; 4]> {
    if let Some(hex) = value.strip_prefix('#')
        && hex.len() == 6
    {
        return Some([
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
            255,
        ]);
    }
    let values = value.strip_prefix("rgba(")?.strip_suffix(')')?;
    let mut values = values.split(',').map(str::trim);
    Some([
        values.next()?.parse().ok()?,
        values.next()?.parse().ok()?,
        values.next()?.parse().ok()?,
        (values.next()?.parse::<f32>().ok()?.clamp(0.0, 1.0) * 255.0).round() as u8,
    ])
}

fn load_font(environment: &str, candidates: &[&str]) -> Option<FontArc> {
    std::env::var_os(environment)
        .and_then(|path| read_font(Path::new(&path)))
        .or_else(|| {
            candidates
                .iter()
                .find_map(|path| read_font(Path::new(path)))
        })
}

fn read_font(path: &Path) -> Option<FontArc> {
    FontArc::try_from_vec(fs::read(path).ok()?).ok()
}

const SANS_REGULAR: &[&str] = &[
    "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
    "/usr/share/fonts/truetype/ubuntu/UbuntuSans-R.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "C:\\Windows\\Fonts\\arial.ttf",
];
const SANS_BOLD: &[&str] = &[
    "/usr/share/fonts/truetype/ubuntu/Ubuntu-B.ttf",
    "/usr/share/fonts/truetype/ubuntu/UbuntuSans-B.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/truetype/liberation2/LiberationSans-Bold.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSansBold.ttf",
    "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    "C:\\Windows\\Fonts\\arialbd.ttf",
];
const SERIF_REGULAR: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
    "/usr/share/fonts/truetype/liberation2/LiberationSerif-Regular.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSerif.ttf",
    "/System/Library/Fonts/Supplemental/Times New Roman.ttf",
    "C:\\Windows\\Fonts\\times.ttf",
];
const SERIF_BOLD: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSerif-Bold.ttf",
    "/usr/share/fonts/truetype/liberation2/LiberationSerif-Bold.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSerifBold.ttf",
    "/System/Library/Fonts/Supplemental/Times New Roman Bold.ttf",
    "C:\\Windows\\Fonts\\timesbd.ttf",
];

#[cfg(test)]
mod tests {
    use super::{create_native_2d_host, css_pixels_to_ab_glyph_height, parse_color, parse_font};
    use flight_sdk::{GlImageSourceToken, SurfaceRef};

    #[test]
    fn parses_flight_font_strings() {
        let title = parse_font("normal bold 96px Georgia, Times New Roman, serif");
        assert!(title.bold);
        assert!(title.serif);
        assert_eq!(title.size, 96.0);
        let button = parse_font("normal normal 13px system-ui, Segoe UI, sans-serif");
        assert!(!button.bold);
        assert!(!button.serif);
        assert_eq!(button.size, 13.0);
    }

    #[test]
    fn parses_flight_rgba_strings() {
        assert_eq!(
            parse_color("rgba(255,209,102,1)"),
            Some([255, 209, 102, 255])
        );
        assert_eq!(parse_color("rgba(0,0,0,0.5)"), Some([0, 0, 0, 128]));
    }

    #[test]
    fn converts_css_em_pixels_to_ab_glyph_line_height() {
        assert_eq!(css_pixels_to_ab_glyph_height(16.0, 1200.0, 1000.0), 19.2);
    }

    #[test]
    fn exposes_mutated_canvas_pixels_to_flight_images() {
        let host = create_native_2d_host();
        let surface = (host.canvas.create_surface)(2.0, 2.0).unwrap();
        let erased: SurfaceRef = surface.clone();
        let image = (host.image.create_image_from_surface.as_ref().unwrap())(&erased);
        let context = surface.read().unwrap().context.handle.clone();
        context.set_fill_style("rgba(255,0,0,1)");
        context.fill_rect(0.0, 0.0, 1.0, 1.0);

        let image = image.read().unwrap();
        assert_eq!(image.width, 2);
        assert_eq!(image.height, 2);
        let token = image
            .source
            .handle
            .downcast_ref::<GlImageSourceToken>()
            .unwrap();
        assert_eq!(&token.pixels[..4], &[255, 0, 0, 255]);
        assert_eq!(&token.pixels[4..], &[0; 12]);
    }

    #[test]
    fn renders_game_emoji_without_a_platform_emoji_font() {
        let host = create_native_2d_host();
        let surface = (host.canvas.create_surface)(32.0, 32.0).unwrap();
        let erased: SurfaceRef = surface.clone();
        let image = (host.image.create_image_from_surface.as_ref().unwrap())(&erased);
        let context = surface.read().unwrap().context.handle.clone();
        context.set_font("normal normal 24px system-ui, sans-serif");
        context.fill_text("🐴", 4.0, 26.0);

        let image = image.read().unwrap();
        let token = image
            .source
            .handle
            .downcast_ref::<GlImageSourceToken>()
            .unwrap();
        assert!(token.pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
        assert!(
            token
                .pixels
                .chunks_exact(4)
                .any(|pixel| pixel[0] > pixel[2] && pixel[3] == 255)
        );
    }
}
