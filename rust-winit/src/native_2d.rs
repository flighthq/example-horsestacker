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
        let Some(font) = self.fonts.select(spec) else {
            return text.chars().count() as f32 * spec.size * 0.56;
        };
        text_width(font, text, spec.size)
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
            let Some(font) = fonts.select(spec) else {
                return;
            };
            draw_text(font, state, width, height, text, x, y);
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
    font: &FontArc,
    state: &mut CanvasState,
    width: u32,
    height: u32,
    text: &str,
    x: f32,
    y: f32,
) {
    let [a, b, c, d, e, f] = state.transform;
    let scale_factor = ((a.abs() + d.abs()) * 0.5).max(0.01);
    let scale = PxScale::from(state.font.size * scale_factor);
    let scaled = font.as_scaled(scale);
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

fn text_width(font: &FontArc, text: &str, size: f32) -> f32 {
    let scaled = font.as_scaled(PxScale::from(size.max(0.01)));
    let mut width = 0.0;
    let mut previous = None;
    for character in text.chars() {
        let id = scaled.glyph_id(character);
        if let Some(previous) = previous {
            width += scaled.kern(previous, id);
        }
        width += scaled.h_advance(id);
        previous = Some(id);
    }
    width
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
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "C:\\Windows\\Fonts\\arial.ttf",
];
const SANS_BOLD: &[&str] = &[
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
    use super::{create_native_2d_host, parse_color, parse_font};
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
}
