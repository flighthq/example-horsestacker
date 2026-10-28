//! Native OpenGL compatibility glue for Flight's WebGL-authored shaders.
//!
//! Flight's GL shader sources deliberately mirror the TypeScript/WebGL sources and begin with
//! `#version 300 es`. A desktop OpenGL context needs the equivalent `#version 330 core` header.
//! Some lower-level Flight render helpers already perform that translation, but the scene 3D
//! material and shadow paths compile their source directly. Intercepting `glShaderSource` keeps the
//! compatibility rule in one native-host boundary and covers every Flight GL renderer uniformly.
//! The reverse translation also keeps Flight's translated helpers working when glutin must fall
//! back to an OpenGL ES context.

use std::cell::Cell;
use std::ffi::{CStr, c_char, c_void};

type GlShaderSource = unsafe extern "system" fn(
    shader: u32,
    count: i32,
    strings: *const *const c_char,
    lengths: *const i32,
);

thread_local! {
    static ORIGINAL_GL_SHADER_SOURCE: Cell<Option<GlShaderSource>> = const { Cell::new(None) };
    static USE_DESKTOP_GLSL: Cell<bool> = const { Cell::new(true) };
}

const WEBGL_VERSION: &[u8] = b"#version 300 es";
const DESKTOP_GL_VERSION: &[u8] = b"#version 330 core";

/// Creates a glow context and makes Flight's WebGL-authored GLSL portable to desktop OpenGL.
///
/// The adapter only replaces a leading GLSL version directive: WebGL/GLES source becomes desktop
/// GLSL for an OpenGL context, while Flight helpers that already emitted desktop GLSL are translated
/// back when glutin selected its GLES fallback.
///
/// # Safety
///
/// `load` must return valid procedure addresses for the current GL context, and that context must
/// remain current for every use of the returned glow context.
pub unsafe fn create_native_gl_context(
    mut load: impl FnMut(&CStr) -> *const c_void,
    desktop_gl: bool,
) -> glow::Context {
    // SAFETY: the caller owns the current native GL context and upholds glow's loader contract.
    unsafe {
        glow::Context::from_loader_function_cstr(|symbol| {
            let address = load(symbol);
            if symbol.to_bytes() == b"glShaderSource" && !address.is_null() {
                // SAFETY: the platform procedure has the OpenGL glShaderSource ABI.
                let original = std::mem::transmute::<*const c_void, GlShaderSource>(address);
                ORIGINAL_GL_SHADER_SOURCE.with(|slot| slot.set(Some(original)));
                USE_DESKTOP_GLSL.with(|slot| slot.set(desktop_gl));
                native_gl_shader_source as *const () as *const c_void
            } else {
                address
            }
        })
    }
}

unsafe extern "system" fn native_gl_shader_source(
    shader: u32,
    count: i32,
    strings: *const *const c_char,
    lengths: *const i32,
) {
    let original = ORIGINAL_GL_SHADER_SOURCE.with(Cell::get);
    let Some(original) = original else {
        return;
    };
    if count <= 0 || strings.is_null() {
        // SAFETY: this forwards the driver's original arguments to the original entry point.
        unsafe { original(shader, count, strings, lengths) };
        return;
    }

    let count = count as usize;
    // SAFETY: OpenGL guarantees `count` readable source pointers and, when present, lengths.
    let source_pointers = unsafe { std::slice::from_raw_parts(strings, count) };
    let source_lengths = if lengths.is_null() {
        None
    } else {
        // SAFETY: OpenGL guarantees one length for every source pointer when lengths is non-null.
        Some(unsafe { std::slice::from_raw_parts(lengths, count) })
    };
    let mut translated = Vec::with_capacity(count);
    for (index, source_pointer) in source_pointers.iter().copied().enumerate() {
        if source_pointer.is_null() {
            // Preserve unusual caller input exactly rather than dereferencing a null string.
            // SAFETY: this forwards the driver's original arguments to the original entry point.
            unsafe { original(shader, count as i32, strings, lengths) };
            return;
        }
        let source = match source_lengths.map(|values| values[index]) {
            Some(length) if length >= 0 => {
                // SAFETY: the caller declared this source buffer to contain `length` bytes.
                unsafe { std::slice::from_raw_parts(source_pointer.cast::<u8>(), length as usize) }
            }
            _ => {
                // SAFETY: a negative or omitted length declares this source NUL-terminated.
                unsafe { CStr::from_ptr(source_pointer) }.to_bytes()
            }
        };
        let desktop_gl = USE_DESKTOP_GLSL.with(Cell::get);
        translated.push(translate_glsl_version(source, desktop_gl));
    }
    if translated
        .iter()
        .any(|source| source.len() > i32::MAX as usize)
    {
        // SAFETY: this forwards the driver's original arguments to the original entry point.
        unsafe { original(shader, count as i32, strings, lengths) };
        return;
    }
    let translated_pointers = translated
        .iter()
        .map(|source| source.as_ptr().cast::<c_char>())
        .collect::<Vec<_>>();
    let translated_lengths = translated
        .iter()
        .map(|source| source.len() as i32)
        .collect::<Vec<_>>();
    // SAFETY: the translated byte buffers and their pointer/length arrays live through this call.
    unsafe {
        original(
            shader,
            count as i32,
            translated_pointers.as_ptr(),
            translated_lengths.as_ptr(),
        )
    };
}

fn translate_glsl_version(source: &[u8], desktop_gl: bool) -> Vec<u8> {
    let (from, to) = if desktop_gl {
        (WEBGL_VERSION, DESKTOP_GL_VERSION)
    } else {
        (DESKTOP_GL_VERSION, WEBGL_VERSION)
    };
    let Some(rest) = source.strip_prefix(from) else {
        return source.to_vec();
    };
    let mut result = Vec::with_capacity(to.len() + rest.len());
    result.extend_from_slice(to);
    result.extend_from_slice(rest);
    result
}

#[cfg(test)]
mod tests {
    use super::translate_glsl_version;

    #[test]
    fn translates_the_webgl_version_for_desktop_gl() {
        assert_eq!(
            translate_glsl_version(b"#version 300 es\nprecision highp float;", true),
            b"#version 330 core\nprecision highp float;"
        );
    }

    #[test]
    fn translates_flight_desktop_helpers_back_for_gles() {
        assert_eq!(
            translate_glsl_version(b"#version 330 core\nvoid main() {}", false),
            b"#version 300 es\nvoid main() {}"
        );
    }

    #[test]
    fn leaves_non_webgl_and_nonleading_directives_unchanged() {
        assert_eq!(
            translate_glsl_version(b"#version 330 core\nvoid main() {}", true),
            b"#version 330 core\nvoid main() {}"
        );
        assert_eq!(
            translate_glsl_version(b"// prefix\n#version 300 es", true),
            b"// prefix\n#version 300 es"
        );
    }
}
