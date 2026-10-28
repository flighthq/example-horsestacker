//! Development framebuffer-edge diagnostics, corresponding to `scene/edgeProbe.ts`.

use glow::{HasContext, PixelPackData};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct EdgeReading {
    changed: usize,
    max_delta: u8,
    samples: Vec<String>,
}

const SAMPLE_COUNT: usize = 5;
const VISIBLE_DELTA: u8 = 4;

/// Reads the four outer device-pixel lines of the currently bound framebuffer.
///
/// The browser obtains the dimensions from its canvas-backed GL context. The native host passes
/// the current framebuffer dimensions explicitly.
pub fn probe_frame_edges(gl: &glow::Context, label: &str, width: i32, height: i32) {
    if width < 2 || height < 2 {
        return;
    }
    unsafe {
        if gl.get_parameter_i32(glow::FRAMEBUFFER_BINDING) != 0 {
            eprintln!(
                "[edge probe: {label}] a render target is still bound; reading that, not the window"
            );
        }
        let bottom = read_strip(gl, 0, 0, width, 2);
        let top = read_strip(gl, 0, height - 2, width, 2);
        let left = read_strip(gl, 0, 0, 2, height);
        let right = read_strip(gl, width - 2, 0, 2, height);
        let readings = [
            ("top", compare_rows(&top, width as usize, 1, 0)),
            ("bottom", compare_rows(&bottom, width as usize, 0, 1)),
            ("left", compare_columns(&left, height as usize, 0, 1)),
            ("right", compare_columns(&right, height as usize, 1, 0)),
        ];
        let summary = readings
            .iter()
            .map(|(edge, reading)| format!("{edge} {}/{}", reading.changed, SAMPLE_COUNT))
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!("[edge probe: {label}] {width}x{height} — {summary}");
        for (edge, reading) in readings {
            let verdict = if reading.max_delta >= VISIBLE_DELTA {
                format!(
                    "DIFFERS from the row inside it (max delta {})",
                    reading.max_delta
                )
            } else {
                "matches the row inside it".to_owned()
            };
            eprintln!("{edge}: {verdict}");
            for sample in reading.samples {
                eprintln!("    {sample}");
            }
        }
    }
}

unsafe fn read_strip(gl: &glow::Context, x: i32, y: i32, width: i32, height: i32) -> Vec<u8> {
    let mut pixels = vec![0; width as usize * height as usize * 4];
    unsafe {
        gl.read_pixels(
            x,
            y,
            width,
            height,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            PixelPackData::Slice(Some(&mut pixels)),
        );
    }
    pixels
}

fn compare_rows(strip: &[u8], width: usize, outer_row: usize, inner_row: usize) -> EdgeReading {
    compare(
        width,
        |index| {
            (
                (outer_row * width + index) * 4,
                (inner_row * width + index) * 4,
            )
        },
        strip,
    )
}

fn compare_columns(
    strip: &[u8],
    height: usize,
    outer_column: usize,
    inner_column: usize,
) -> EdgeReading {
    compare(
        height,
        |index| {
            (
                (index * 2 + outer_column) * 4,
                (index * 2 + inner_column) * 4,
            )
        },
        strip,
    )
}

fn compare(
    span: usize,
    offsets_at: impl Fn(usize) -> (usize, usize),
    pixels: &[u8],
) -> EdgeReading {
    let mut reading = EdgeReading::default();
    for sample in 0..SAMPLE_COUNT {
        let index =
            ((((sample as f64 + 0.5) / SAMPLE_COUNT as f64) * (span - 1) as f64).round()) as usize;
        let (outer, inner) = offsets_at(index);
        let delta = (0..4)
            .map(|channel| pixels[outer + channel].abs_diff(pixels[inner + channel]))
            .max()
            .unwrap_or(0);
        reading.max_delta = reading.max_delta.max(delta);
        if delta >= VISIBLE_DELTA {
            reading.changed += 1;
        }
        let suffix = if delta >= VISIBLE_DELTA {
            format!("  (delta {delta})")
        } else {
            String::new()
        };
        reading.samples.push(format!(
            "at {index}: inner {} -> outer {}{suffix}",
            format_pixel(pixels, inner),
            format_pixel(pixels, outer)
        ));
    }
    reading
}

fn format_pixel(pixels: &[u8], offset: usize) -> String {
    format!(
        "rgba({:>3}, {:>3}, {:>3}, {:>3})",
        pixels[offset],
        pixels[offset + 1],
        pixels[offset + 2],
        pixels[offset + 3]
    )
}
