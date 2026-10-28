//! Native OpenGL compatibility glue for Flight's WebGL-authored shaders.
//!
//! Flight's GL shader sources deliberately mirror the TypeScript/WebGL sources. Most use GLSL ES
//! 3.00, while a few Flight 2D paths still use versionless WebGL 1 syntax. A desktop OpenGL context
//! needs the equivalent GLSL 3.30 syntax and cannot parse ES precision declarations. Some
//! lower-level Flight render helpers already translate the version header, but the scene 3D and
//! shape paths compile their source directly. Intercepting `glShaderSource` keeps the compatibility
//! rule in one native-host boundary and covers every Flight GL renderer uniformly. The reverse
//! version translation also keeps Flight's translated helpers working when glutin must fall back to
//! an OpenGL ES context.

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
/// For desktop OpenGL the adapter normalizes the version, removes ES precision declarations and
/// upgrades versionless WebGL 1 keywords. For GLES it only restores a version header that Flight's
/// existing native helper may already have changed to desktop GLSL.
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
        translated.push(translate_glsl_source(source, desktop_gl));
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

fn translate_glsl_source(source: &[u8], desktop_gl: bool) -> Vec<u8> {
    if !desktop_gl {
        let Some(rest) = source.strip_prefix(DESKTOP_GL_VERSION) else {
            return source.to_vec();
        };
        let mut result = Vec::with_capacity(WEBGL_VERSION.len() + rest.len());
        result.extend_from_slice(WEBGL_VERSION);
        result.extend_from_slice(rest);
        return result;
    }

    let Ok(source) = std::str::from_utf8(source) else {
        return source.to_vec();
    };
    let legacy_fragment = contains_identifier(source, "gl_FragColor");
    let legacy_vertex = contains_identifier(source, "attribute");
    let body = remove_host_specific_declarations(source);
    let body = rewrite_desktop_identifiers(&body, legacy_fragment, legacy_vertex);

    let mut result = String::with_capacity(DESKTOP_GL_VERSION.len() + body.len() + 64);
    result.push_str("#version 330 core\n");
    if legacy_fragment {
        result.push_str("layout(location = 0) out vec4 flight_FragColor;\n");
    }
    result.push_str(body.trim_start_matches(['\r', '\n']));
    result.into_bytes()
}

fn remove_host_specific_declarations(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    for line in source.split_inclusive('\n') {
        let declaration = line.trim();
        if declaration == "#version 300 es"
            || declaration == "#version 330 core"
            || declaration.starts_with("precision ") && declaration.ends_with(';')
        {
            continue;
        }
        result.push_str(line);
    }
    result
}

fn rewrite_desktop_identifiers(source: &str, legacy_fragment: bool, legacy_vertex: bool) -> String {
    let bytes = source.as_bytes();
    let mut result = String::with_capacity(source.len());
    let mut at = 0;
    while at < bytes.len() {
        if !is_identifier_start(bytes[at]) {
            result.push(bytes[at] as char);
            at += 1;
            continue;
        }
        let start = at;
        at += 1;
        while at < bytes.len() && is_identifier_continue(bytes[at]) {
            at += 1;
        }
        let identifier = &source[start..at];
        match identifier {
            "lowp" | "mediump" | "highp" => {}
            "attribute" if legacy_vertex => result.push_str("in"),
            "varying" if legacy_vertex => result.push_str("out"),
            "varying" if legacy_fragment => result.push_str("in"),
            "texture2D" | "textureCube" => result.push_str("texture"),
            "gl_FragColor" if legacy_fragment => result.push_str("flight_FragColor"),
            _ => result.push_str(identifier),
        }
    }
    result
}

fn contains_identifier(source: &str, expected: &str) -> bool {
    let bytes = source.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if !is_identifier_start(bytes[at]) {
            at += 1;
            continue;
        }
        let start = at;
        at += 1;
        while at < bytes.len() && is_identifier_continue(bytes[at]) {
            at += 1;
        }
        if &source[start..at] == expected {
            return true;
        }
    }
    false
}

const fn is_identifier_start(value: u8) -> bool {
    value == b'_' || value.is_ascii_alphabetic()
}

const fn is_identifier_continue(value: u8) -> bool {
    is_identifier_start(value) || value.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::translate_glsl_source;

    #[test]
    fn translates_the_webgl_version_for_desktop_gl() {
        assert_eq!(
            translate_glsl_source(b"#version 300 es\nprecision highp float;", true),
            b"#version 330 core\n"
        );
    }

    #[test]
    fn upgrades_the_actual_flight_shape_mesh_shader_forms() {
        assert_eq!(
            String::from_utf8(translate_glsl_source(
                b"\nattribute vec2 a_position;\nuniform mat3 u_matrix;\nvoid main() { gl_Position = vec4(a_position, 0.0, 1.0); }\n",
                true,
            ))
            .unwrap(),
            "#version 330 core\nin vec2 a_position;\nuniform mat3 u_matrix;\nvoid main() { gl_Position = vec4(a_position, 0.0, 1.0); }\n"
        );
        assert_eq!(
            String::from_utf8(translate_glsl_source(
                b"\nprecision mediump float;\nuniform vec4 u_color;\nvoid main() { gl_FragColor = u_color; }\n",
                true,
            ))
            .unwrap(),
            "#version 330 core\nlayout(location = 0) out vec4 flight_FragColor;\nuniform vec4 u_color;\nvoid main() { flight_FragColor = u_color; }\n"
        );
    }

    #[test]
    fn removes_es_precision_qualifiers_from_modern_desktop_glsl() {
        assert_eq!(
            String::from_utf8(translate_glsl_source(
                b"#version 330 core\nprecision highp float;\nuniform highp sampler2D source;\n",
                true,
            ))
            .unwrap(),
            "#version 330 core\nuniform  sampler2D source;\n"
        );
    }

    #[test]
    fn translates_flight_desktop_helpers_back_for_gles() {
        assert_eq!(
            translate_glsl_source(b"#version 330 core\nvoid main() {}", false),
            b"#version 300 es\nvoid main() {}"
        );
    }

    #[test]
    fn normalizes_existing_and_nonleading_version_directives() {
        assert_eq!(
            translate_glsl_source(b"#version 330 core\nvoid main() {}", true),
            b"#version 330 core\nvoid main() {}"
        );
        assert_eq!(
            translate_glsl_source(b"// prefix\n#version 300 es", true),
            b"#version 330 core\n// prefix\n"
        );
    }
}
