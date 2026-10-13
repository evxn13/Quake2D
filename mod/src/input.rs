//! Contrôles « twin-stick » lus directement dans SDL (déjà chargé par le jeu) :
//! clavier = déplacement par rapport à l'écran, souris = viseur.

use core::ffi::{c_int, c_void};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicI32, Ordering};

// Scancodes SDL (positions physiques : WASD en QWERTY = ZQSD en AZERTY).
const SC_A: usize = 4;
const SC_D: usize = 7;
const SC_S: usize = 22;
const SC_W: usize = 26;
const SC_RIGHT: usize = 79;
const SC_LEFT: usize = 80;
const SC_DOWN: usize = 81;
const SC_UP: usize = 82;
const SC_LSHIFT: usize = 225;
const SC_RSHIFT: usize = 229;

const SDL_MOUSEMOTION: u32 = 0x400;

type GetKeyboardState = unsafe extern "C" fn(*mut c_int) -> *const u8;
type EventFilter = unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int;
type AddEventWatch = unsafe extern "C" fn(EventFilter, *mut c_void);

static KEYBOARD_STATE: OnceLock<Option<GetKeyboardState>> = OnceLock::new();

/// Viseur par rapport au centre de la vue (= le joueur), en 1/256 de pixel.
static CURSOR_X: AtomicI32 = AtomicI32::new(96 * 256);
static CURSOR_Y: AtomicI32 = AtomicI32::new(0);
static HALF_W: AtomicI32 = AtomicI32::new(320);
static HALF_H: AtomicI32 = AtomicI32::new(200);
/// Sensibilité en 1/256 de pixel de vue par pixel de souris.
static SENS: AtomicI32 = AtomicI32::new(256);

unsafe fn sym<T>(name: &core::ffi::CStr) -> Option<T> {
    let p = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
    (!p.is_null()).then(|| unsafe { core::mem::transmute_copy(&p) })
}

/// Branche l'observation des événements SDL (une seule fois, SDL initialisé).
fn keyboard_state() -> Option<GetKeyboardState> {
    *KEYBOARD_STATE.get_or_init(|| unsafe {
        let add_watch: AddEventWatch = sym(c"SDL_AddEventWatch")?;
        add_watch(on_event, core::ptr::null_mut());
        sym(c"SDL_GetKeyboardState")
    })
}

/// Observe chaque événement SDL sans le consommer : le jeu le reçoit aussi.
unsafe extern "C" fn on_event(_user: *mut c_void, event: *mut c_void) -> c_int {
    // SDL_MouseMotionEvent : type@0 … xrel@28, yrel@32.
    let ev = event as *const u8;
    if unsafe { *(ev as *const u32) } == SDL_MOUSEMOTION {
        let (xrel, yrel) = unsafe { (*(ev.add(28) as *const i32), *(ev.add(32) as *const i32)) };
        let s = SENS.load(Ordering::Relaxed);
        move_cursor(xrel * s, yrel * s);
    }
    1
}

fn move_cursor(dx: i32, dy: i32) {
    let hw = HALF_W.load(Ordering::Relaxed) * 256;
    let hh = HALF_H.load(Ordering::Relaxed) * 256;
    CURSOR_X.store((CURSOR_X.load(Ordering::Relaxed) + dx).clamp(-hw, hw), Ordering::Relaxed);
    CURSOR_Y.store((CURSOR_Y.load(Ordering::Relaxed) + dy).clamp(-hh, hh), Ordering::Relaxed);
}

pub fn set_sensitivity(s: f32) {
    SENS.store((s * 256.0) as i32, Ordering::Relaxed);
}

pub fn set_view_size(w: i32, h: i32) {
    HALF_W.store(w / 2, Ordering::Relaxed);
    HALF_H.store(h / 2, Ordering::Relaxed);
    move_cursor(0, 0);
}

/// Viseur en pixels, relatif au centre de la vue (y vers le bas).
pub fn cursor() -> (f32, f32) {
    (CURSOR_X.load(Ordering::Relaxed) as f32 / 256.0, CURSOR_Y.load(Ordering::Relaxed) as f32 / 256.0)
}

/// Direction voulue en coordonnées écran (x à droite, y vers le haut) et marche.
#[derive(Clone, Copy, Default)]
pub struct Movement {
    pub x: f32,
    pub y: f32,
    pub walk: bool,
}

pub fn read_movement() -> Option<Movement> {
    let get = keyboard_state()?;
    let mut n: c_int = 0;
    let keys = unsafe { get(&mut n) };
    if keys.is_null() {
        return None;
    }
    let keys = unsafe { core::slice::from_raw_parts(keys, n as usize) };
    let down = |sc: usize| keys.get(sc).is_some_and(|&k| k != 0);
    let axis = |neg: bool, pos: bool| (pos as i32 - neg as i32) as f32;
    Some(Movement {
        x: axis(down(SC_A) || down(SC_LEFT), down(SC_D) || down(SC_RIGHT)),
        y: axis(down(SC_S) || down(SC_DOWN), down(SC_W) || down(SC_UP)),
        walk: down(SC_LSHIFT) || down(SC_RSHIFT),
    })
}

/// Lacet (degrés, 0 = est, 90 = nord) du joueur vers le viseur.
/// La caméra regarde vers le bas avec le nord en haut de l'écran.
pub fn aim_yaw(cursor: (f32, f32)) -> f32 {
    (-cursor.1).atan2(cursor.0).to_degrees()
}

/// (avant, côté) pour aller dans la direction écran demandée en regardant `yaw`.
pub fn to_player_frame(m: Movement, yaw_deg: f32, speed: f32) -> (f32, f32) {
    let len = (m.x * m.x + m.y * m.y).sqrt();
    if len == 0.0 {
        return (0.0, 0.0);
    }
    let (mx, my) = (m.x / len, m.y / len);
    let (s, c) = yaw_deg.to_radians().sin_cos();
    // Repère du joueur : avant = (cos, sin), droite = (sin, -cos).
    ((mx * c + my * s) * speed, (mx * s - my * c) * speed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viseur_a_droite_vise_est_en_haut_vise_nord() {
        assert!(aim_yaw((10.0, 0.0)).abs() < 1e-4);
        assert!((aim_yaw((0.0, -10.0)) - 90.0).abs() < 1e-4);
    }

    #[test]
    fn haut_avance_vers_le_nord_meme_en_regardant_a_l_est() {
        let up = Movement { y: 1.0, ..Default::default() };
        let (f, s) = to_player_frame(up, 0.0, 400.0);
        assert!(f.abs() < 1e-3 && (s + 400.0).abs() < 1e-3); // pas chassé à gauche
        let (f, s) = to_player_frame(up, 90.0, 400.0);
        assert!((f - 400.0).abs() < 1e-3 && s.abs() < 1e-3);
    }
}
