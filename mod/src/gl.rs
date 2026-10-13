//! Les quelques fonctions OpenGL (profil de compatibilité) utilisées par le mod,
//! résolues dans la bibliothèque GL déjà chargée par le jeu.

use std::sync::OnceLock;

const GL_PROJECTION: u32 = 0x1701;
const GL_MODELVIEW: u32 = 0x1700;
const GL_LINES: u32 = 0x0001;
const GL_DEPTH_TEST: u32 = 0x0B71;
const GL_TEXTURE_2D: u32 = 0x0DE1;
const GL_BLEND: u32 = 0x0BE2;
const GL_SRC_ALPHA: u32 = 0x0302;
const GL_ONE_MINUS_SRC_ALPHA: u32 = 0x0303;
const GL_ALL_ATTRIB_BITS: u32 = 0x000F_FFFF;

struct Gl {
    ortho: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64),
    frustum: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64),
    clear_color: unsafe extern "C" fn(f32, f32, f32, f32),
    enable: unsafe extern "C" fn(u32),
    blend_func: unsafe extern "C" fn(u32, u32),
    matrix_mode: unsafe extern "C" fn(u32),
    push_matrix: unsafe extern "C" fn(),
    pop_matrix: unsafe extern "C" fn(),
    load_identity: unsafe extern "C" fn(),
    push_attrib: unsafe extern "C" fn(u32),
    pop_attrib: unsafe extern "C" fn(),
    disable: unsafe extern "C" fn(u32),
    color4f: unsafe extern "C" fn(f32, f32, f32, f32),
    begin: unsafe extern "C" fn(u32),
    end: unsafe extern "C" fn(),
    vertex2f: unsafe extern "C" fn(f32, f32),
}

fn gl() -> Option<&'static Gl> {
    static GL: OnceLock<Option<Gl>> = OnceLock::new();
    GL.get_or_init(|| {
        fn f<T>(name: &core::ffi::CStr) -> Option<T> {
            let p = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
            (!p.is_null()).then(|| unsafe { core::mem::transmute_copy(&p) })
        }
        Some(Gl {
            ortho: f(c"glOrtho")?,
            frustum: f(c"glFrustum")?,
            clear_color: f(c"glClearColor")?,
            enable: f(c"glEnable")?,
            blend_func: f(c"glBlendFunc")?,
            matrix_mode: f(c"glMatrixMode")?,
            push_matrix: f(c"glPushMatrix")?,
            pop_matrix: f(c"glPopMatrix")?,
            load_identity: f(c"glLoadIdentity")?,
            push_attrib: f(c"glPushAttrib")?,
            pop_attrib: f(c"glPopAttrib")?,
            disable: f(c"glDisable")?,
            color4f: f(c"glColor4f")?,
            begin: f(c"glBegin")?,
            end: f(c"glEnd")?,
            vertex2f: f(c"glVertex2f")?,
        })
    })
    .as_ref()
}

pub fn ortho(l: f32, r: f32, b: f32, t: f32, near: f32, far: f32) {
    if let Some(gl) = gl() {
        unsafe { (gl.ortho)(l as f64, r as f64, b as f64, t as f64, near as f64, far as f64) };
    }
}

pub fn frustum(l: f32, r: f32, b: f32, t: f32, near: f32, far: f32) {
    if let Some(gl) = gl() {
        unsafe { (gl.frustum)(l as f64, r as f64, b as f64, t as f64, near as f64, far as f64) };
    }
}

pub fn clear_color(r: f32, g: f32, b: f32) {
    if let Some(gl) = gl() {
        unsafe { (gl.clear_color)(r, g, b, 0.0) };
    }
}

/// Viseur en croix à `cursor` (pixels depuis le centre de la vue, y vers le bas).
/// Dessiné dans la zone de vue 3D, sans toucher à l'état GL du jeu.
pub fn draw_crosshair(w: i32, h: i32, cursor: (f32, f32), alpha: f32) {
    let Some(gl) = gl() else { return };
    let (sx, sy) = (2.0 / w.max(1) as f32, 2.0 / h.max(1) as f32);
    let (x, y) = (cursor.0 * sx, -cursor.1 * sy);
    unsafe {
        (gl.push_attrib)(GL_ALL_ATTRIB_BITS);
        (gl.disable)(GL_DEPTH_TEST);
        (gl.disable)(GL_TEXTURE_2D);
        (gl.enable)(GL_BLEND);
        (gl.blend_func)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
        (gl.matrix_mode)(GL_PROJECTION);
        (gl.push_matrix)();
        (gl.load_identity)();
        (gl.matrix_mode)(GL_MODELVIEW);
        (gl.push_matrix)();
        (gl.load_identity)();
        (gl.color4f)(1.0, 1.0, 1.0, alpha);
        (gl.begin)(GL_LINES);
        for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            (gl.vertex2f)(x + dx * 3.0 * sx, y + dy * 3.0 * sy);
            (gl.vertex2f)(x + dx * 8.0 * sx, y + dy * 8.0 * sy);
        }
        (gl.end)();
        (gl.pop_matrix)();
        (gl.matrix_mode)(GL_PROJECTION);
        (gl.pop_matrix)();
        (gl.matrix_mode)(GL_MODELVIEW);
        (gl.pop_attrib)();
    }
}
