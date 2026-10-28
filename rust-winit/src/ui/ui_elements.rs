//! Flight 2D UI helpers and palette, corresponding to `ui/uiElements.ts`.

use std::sync::{Arc, RwLock};

use flight_sdk::{
    CapsStyle, JointStyle, Node2D, NodeRef, Ref, Shape, TextFormat, TextFormatAlign, TextLabel,
    append_shape_begin_fill, append_shape_line_style, append_shape_line_to, append_shape_move_to,
    append_shape_polygon, append_shape_quadratic_curve_to, append_shape_rounded_rectangle,
    clear_shape_commands, create_text_label, invalidate_node_appearance,
    invalidate_node_local_transform, invalidate_node_render, set_text_label_format,
    set_text_label_height, set_text_label_string, set_text_label_width,
};

pub const GOLD: u32 = 0xffd166ff;
pub const INK: u32 = 0xfbf7ecff;
pub const SERIF: &str = "Georgia, Times New Roman, serif";
pub const SANS: &str = "system-ui, Segoe UI, Helvetica, Arial, sans-serif";

pub fn label(text: &str, size: f32, color: u32, font: &str, bold: bool) -> Ref<TextLabel> {
    let node = create_text_label(None);
    set_text_label_string(&node, text);
    set_text_label_format(
        &node,
        Arc::new(RwLock::new(TextFormat {
            align: Some(TextFormatAlign::Center),
            bold: Some(bold),
            color: Some(color),
            font: Some(font.to_owned()),
            size: Some(size),
            ..TextFormat::default()
        })),
    );
    set_text_label_width(&node, 10.0);
    set_text_label_height(&node, size * 1.6);
    node
}

pub fn place<T: Node2D + 'static>(node: &Ref<T>, x: f32, y: f32) {
    {
        let mut node = node.write().unwrap();
        node.node2d_fields_mut().x = x;
        node.node2d_fields_mut().y = y;
    }
    let node: NodeRef = node.clone();
    invalidate_node_local_transform(&node);
}

pub fn show<T: Node2D + 'static>(node: &Ref<T>, visible: bool) {
    {
        let mut node = node.write().unwrap();
        if node.node2d_fields().visible == visible {
            return;
        }
        node.node2d_fields_mut().visible = visible;
    }
    let node: NodeRef = node.clone();
    invalidate_node_appearance(&node);
}

pub fn set_text(node: &Ref<TextLabel>, text: &str) {
    if node.read().unwrap().data.text == text {
        return;
    }
    set_text_label_string(node, text);
}

pub fn fill(
    shape: &Ref<Shape>,
    colour: u32,
    alpha: f64,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
) {
    clear_shape_commands(shape);
    append_shape_begin_fill(shape, colour, Some(alpha));
    append_shape_rounded_rectangle(shape, x, y, width, height, radius);
    let node: NodeRef = shape.clone();
    invalidate_node_render(&node);
}

fn measure_text_width(text: &str, size: f32, bold: bool) -> f32 {
    let width_factor = if bold { 0.61 } else { 0.56 };
    text.chars().count() as f32 * size * width_factor
}

pub fn fit_label_to_width(
    node: &Ref<TextLabel>,
    max_size: f32,
    min_size: f32,
    max_width: f32,
) -> f32 {
    let (current_size, text, mut format) = {
        let node = node.read().unwrap();
        let format = node.data.text_format.read().unwrap();
        (
            format.size.unwrap_or(max_size),
            node.data.text.clone(),
            format.clone(),
        )
    };
    if text.is_empty() || max_width <= 0.0 {
        return current_size;
    }
    let width_at_max = measure_text_width(&text, max_size, format.bold.unwrap_or(false));
    let fitted = if width_at_max <= max_width || width_at_max == 0.0 {
        max_size
    } else {
        min_size.max((max_size * max_width / width_at_max).floor())
    };
    if fitted != current_size {
        format.size = Some(fitted);
        set_text_label_format(node, Arc::new(RwLock::new(format)));
        set_text_label_height(node, fitted * 1.6);
    }
    fitted
}

pub fn draw_speaker(shape: &Ref<Shape>, colour: u32, alpha: f64, muted: bool) {
    clear_shape_commands(shape);
    append_shape_line_style(
        shape,
        Some(1.5),
        colour,
        Some(alpha),
        Some(false),
        None,
        Some(CapsStyle::Round),
        Some(JointStyle::Round),
        None,
    );
    append_shape_polygon(
        shape,
        &[
            -6.0, -2.5, -3.0, -2.5, 1.0, -6.5, 1.0, 6.5, -3.0, 2.5, -6.0, 2.5,
        ],
    );
    if muted {
        append_shape_move_to(shape, 4.5, -2.0);
        append_shape_line_to(shape, 8.0, 2.0);
        append_shape_move_to(shape, 8.0, -2.0);
        append_shape_line_to(shape, 4.5, 2.0);
    } else {
        append_shape_move_to(shape, 4.0, -3.0);
        append_shape_quadratic_curve_to(shape, 6.4, 0.0, 4.0, 3.0);
        append_shape_move_to(shape, 7.0, -5.5);
        append_shape_quadratic_curve_to(shape, 10.2, 0.0, 7.0, 5.5);
    }
    let node: NodeRef = shape.clone();
    invalidate_node_render(&node);
}

pub fn draw_fullscreen(shape: &Ref<Shape>, colour: u32, alpha: f64) {
    clear_shape_commands(shape);
    append_shape_line_style(
        shape,
        Some(1.5),
        colour,
        Some(alpha),
        Some(true),
        None,
        Some(CapsStyle::Square),
        Some(JointStyle::Miter),
        None,
    );
    let outer = 6.0;
    let inner = 2.0;
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        append_shape_move_to(shape, sx * inner, sy * outer);
        append_shape_line_to(shape, sx * outer, sy * outer);
        append_shape_line_to(shape, sx * outer, sy * inner);
    }
    let node: NodeRef = shape.clone();
    invalidate_node_render(&node);
}
