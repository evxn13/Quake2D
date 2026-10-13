//! quake2d : Quake (QuakeSpasm) en vue du dessus.
//!
//! Chargé via `LD_PRELOAD`. On garde le moteur de rendu du jeu et on ne
//! change que sa caméra, en remplaçant quelques fonctions :
//! - `R_SetFrustum` : la caméra regarde vers le bas (nord en haut de l'écran)
//!   et les plans d'élimination deviennent les bords du rectangle visible —
//!   le moteur ne traite plus que ce qui est à l'écran ;
//! - `GL_SetFrustum` : projection orthographique, coupée juste au-dessus de la
//!   tête du joueur (plafonds et étages supérieurs disparaissent) ;
//! - `CL_BaseMove` : déplacement au clavier relatif à l'écran, regard vers le
//!   viseur de la souris ;
//! - `V_RenderView` : encadrée pour dessiner le viseur après la vue 3D ;
//! - `Chase_UpdateForDrawing` : neutralisée, pour activer la caméra de
//!   poursuite (qui fait dessiner le modèle du joueur) sans déplacer la caméra.
//!
//! Variables d'environnement :
//! - `QUAKE2D_VIEW`       largeur de la vue en unités de carte (défaut 1000)
//! - `QUAKE2D_CLIP`       hauteur de coupe au-dessus des yeux (défaut 40)
//! - `QUAKE2D_MOUSE_SENS` pixels de viseur par pixel de souris (défaut 1)
//! - `QUAKE2D_DEBUG`      afficher les commandes envoyées au jeu
//! - `QUAKE2D_SHOTS`      numéros d'images où prendre une capture, ex. "100,300"
//! - `QUAKE2D_QUIT_AFTER` quitter le jeu après N images (tests)

mod gl;
mod input;
mod module;
mod symbols;

use core::ffi::c_char;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use module::{JumpHook, MainModule};

macro_rules! addresses {
    ($( $field:ident = $symbol:literal; )*) => {
        struct Addresses { $( $field: usize, )* }

        impl Addresses {
            fn from_symbols(base: usize, symbols: &HashMap<String, u64>) -> Option<Addresses> {
                Some(Addresses { $( $field: base + *symbols.get($symbol)? as usize, )* })
            }
        }
    };
}

addresses! {
    r_set_frustum = "R_SetFrustum";
    gl_set_frustum = "GL_SetFrustum";
    cl_base_move = "CL_BaseMove";
    v_render_view = "V_RenderView";
    chase_update = "Chase_UpdateForDrawing";
    angle_vectors = "AngleVectors";
    cbuf_add_text = "Cbuf_AddText";
    r_refdef = "r_refdef";
    r_origin = "r_origin";
    vpn = "vpn";
    vright = "vright";
    vup = "vup";
    frustum = "frustum";
    cl = "cl";
}

/// `refdef_t` (partiel).
#[repr(C)]
struct Refdef {
    vrect: [i32; 4], // x, y, largeur, hauteur
    _skip: [u8; 104],
    vieworg: [f32; 3],
    viewangles: [f32; 3],
}

/// `mplane_t`.
#[repr(C)]
struct Plane {
    normal: [f32; 3],
    dist: f32,
    kind: u8,
    signbits: u8,
    _pad: [u8; 2],
}

/// `usercmd_t`.
#[repr(C)]
struct UserCmd {
    viewangles: [f32; 3],
    forwardmove: f32,
    sidemove: f32,
    upmove: f32,
}

/// Décalage de `viewangles` dans `client_state_t cl`.
const CL_VIEWANGLES: usize = 0x1bc;
const PLANE_ANYZ: u8 = 5;
/// Tangage 90° = regarder vers le bas ; lacet 90° = nord en haut de l'écran.
const TOP_DOWN: [f32; 3] = [90.0, 90.0, 0.0];

const _: () = {
    assert!(core::mem::offset_of!(Refdef, vieworg) == 120);
    assert!(core::mem::offset_of!(Refdef, viewangles) == 132);
    assert!(size_of::<Plane>() == 20);
    assert!(size_of::<UserCmd>() == 24);
};

struct Settings {
    view_units: f32,
    clip_above: f32,
    shots: Vec<u64>,
    quit_after: Option<u64>,
    debug: bool,
}

struct State {
    addr: Addresses,
    settings: Settings,
    render_view: JumpHook,
}

type AngleVectorsFn = unsafe extern "C" fn(*const f32, *mut f32, *mut f32, *mut f32);
type CbufAddText = unsafe extern "C" fn(*const c_char);

static STATE: OnceLock<State> = OnceLock::new();
static CONFIGURED: AtomicBool = AtomicBool::new(false);
static FRAME: AtomicU64 = AtomicU64::new(0);

#[used]
#[unsafe(link_section = ".init_array")]
static INIT: extern "C" fn() = init;

extern "C" fn init() {
    let Some(module) = MainModule::find() else { return };
    // Pas QuakeSpasm (ou binaire strippé) : on ne touche à rien.
    let Some(addr) = Addresses::from_symbols(module.base, &symbols::load_own()) else { return };
    let env = |k: &str, d: f32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    input::set_sensitivity(env("QUAKE2D_MOUSE_SENS", 1.0));
    let var = |k: &str| std::env::var(k).ok();
    let settings = Settings {
        view_units: env("QUAKE2D_VIEW", 1000.0),
        clip_above: env("QUAKE2D_CLIP", 40.0),
        shots: var("QUAKE2D_SHOTS").map(|v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect()).unwrap_or_default(),
        quit_after: var("QUAKE2D_QUIT_AFTER").and_then(|v| v.parse().ok()),
        debug: var("QUAKE2D_DEBUG").is_some(),
    };

    let hooks = unsafe {
        (|| {
            JumpHook::install(&module, addr.r_set_frustum, set_cull_frustum as extern "C" fn(f32, f32) as usize)?;
            JumpHook::install(&module, addr.gl_set_frustum, set_gl_frustum as extern "C" fn(f32, f32) as usize)?;
            JumpHook::install(&module, addr.cl_base_move, base_move as extern "C" fn(*mut UserCmd) as usize)?;
            JumpHook::install(&module, addr.chase_update, keep_camera as extern "C" fn() as usize)?;
            JumpHook::install(&module, addr.v_render_view, render_view as extern "C" fn() as usize)
        })()
    };
    match hooks {
        Ok(render_view) => {
            let _ = STATE.set(State { addr, settings, render_view });
            eprintln!("[quake2d] vue du dessus activée");
        }
        Err(e) => eprintln!("[quake2d] échec d'un détour : {e}"),
    }
}

unsafe fn var<T>(addr: usize) -> &'static mut T {
    unsafe { &mut *(addr as *mut T) }
}

/// Taille de la vue en unités de carte (largeur, hauteur).
fn view_extent(st: &State) -> (f32, f32) {
    let refdef = unsafe { var::<Refdef>(st.addr.r_refdef) };
    let (w, h) = (refdef.vrect[2].max(1) as f32, refdef.vrect[3].max(1) as f32);
    (st.settings.view_units, st.settings.view_units * h / w)
}

/// Réglages du jeu adaptés à la vue du dessus : modèle du joueur visible
/// (caméra de poursuite), pas d'arme devant la caméra, visée verticale
/// automatique (on ne peut plus viser en hauteur), et souris neutralisée pour
/// le jeu (le viseur la lit directement dans SDL).
fn configure_game(st: &State) {
    console(st, c"chase_active 1; r_drawviewmodel 0; crosshair 0; sv_aim 0.93; sensitivity 0\n");
}

/// Remplace `Chase_UpdateForDrawing` : la caméra reste aux yeux du joueur.
extern "C" fn keep_camera() {}

/// Ajoute une commande à la console du jeu (exécutée à la prochaine image).
fn console(st: &State, cmd: &core::ffi::CStr) {
    unsafe {
        let add: CbufAddText = core::mem::transmute(st.addr.cbuf_add_text);
        add(cmd.as_ptr());
    }
}

/// Remplace `R_SetFrustum`, appelée par R_SetupView juste après le calcul de la
/// visibilité (PVS) depuis les yeux du joueur et avant la caméra OpenGL.
extern "C" fn set_cull_frustum(_fovx: f32, _fovy: f32) {
    let Some(st) = STATE.get() else { return };
    if !CONFIGURED.swap(true, Ordering::Relaxed) {
        configure_game(st);
    }
    let a = &st.addr;
    // Caméra vers le bas : R_SetupGL lira ces angles. Le jeu les recalcule à
    // chaque image, rien à restaurer. Le son, spatialisé avec vpn/vright/vup,
    // suit alors l'écran (un bruit à l'est sort à droite).
    unsafe {
        var::<Refdef>(a.r_refdef).viewangles = TOP_DOWN;
        let angle_vectors: AngleVectorsFn = core::mem::transmute(a.angle_vectors);
        angle_vectors(TOP_DOWN.as_ptr(), a.vpn as *mut f32, a.vright as *mut f32, a.vup as *mut f32);
    }

    // Élimination : les 4 bords du rectangle visible (plus une marge).
    let (w, h) = view_extent(st);
    let origin = unsafe { var::<[f32; 3]>(a.r_origin) };
    let planes = unsafe { var::<[Plane; 4]>(a.frustum) };
    let (hw, hh) = (w / 2.0 + 32.0, h / 2.0 + 32.0);
    let (x, y) = (origin[0], origin[1]);
    let defs = [([1.0, 0.0, 0.0], x - hw), ([-1.0, 0.0, 0.0], -(x + hw)), ([0.0, 1.0, 0.0], y - hh), ([0.0, -1.0, 0.0], -(y + hh))];
    for (plane, (normal, dist)) in planes.iter_mut().zip(defs) {
        plane.normal = normal;
        plane.dist = dist;
        plane.kind = PLANE_ANYZ;
        plane.signbits = (0..3).fold(0, |b, i| b | (((normal[i] < 0.0) as u8) << i));
    }
}

/// Remplace `GL_SetFrustum` : projection orthographique coupée au-dessus des yeux.
extern "C" fn set_gl_frustum(_fovx: f32, _fovy: f32) {
    let Some(st) = STATE.get() else { return };
    let (w, h) = view_extent(st);
    // La caméra est aux yeux du joueur et regarde vers le bas : un « near »
    // négatif garde ce qui est jusqu'à `clip_above` au-dessus d'elle.
    gl::ortho(-w / 2.0, w / 2.0, -h / 2.0, h / 2.0, -st.settings.clip_above, 8192.0);
}

/// Remplace `CL_BaseMove` : déplacement relatif à l'écran, regard vers le viseur.
extern "C" fn base_move(cmd: *mut UserCmd) {
    let Some(st) = STATE.get() else { return };
    let cmd = unsafe { &mut *cmd };
    *cmd = UserCmd { viewangles: [0.0; 3], forwardmove: 0.0, sidemove: 0.0, upmove: 0.0 };
    let Some(m) = input::read_movement() else { return };
    let yaw = input::aim_yaw(input::cursor());
    // CL_SendMove envoie cl.viewangles au serveur.
    unsafe { *var::<[f32; 3]>(st.addr.cl + CL_VIEWANGLES) = [0.0, yaw, 0.0] };
    let speed = if m.walk { 200.0 } else { 400.0 };
    (cmd.forwardmove, cmd.sidemove) = input::to_player_frame(m, yaw, speed);
    if st.settings.debug && (m.x != 0.0 || m.y != 0.0) {
        eprintln!("[quake2d] lacet {yaw:.0}° avant {:.0} côté {:.0}", cmd.forwardmove, cmd.sidemove);
    }
}

/// Encadre `V_RenderView` pour dessiner le viseur sur la vue 3D.
extern "C" fn render_view() {
    let Some(st) = STATE.get() else { return };
    unsafe {
        st.render_view.call_original(|f| {
            let f: extern "C" fn() = core::mem::transmute(f);
            f();
        });
    }
    let refdef = unsafe { var::<Refdef>(st.addr.r_refdef) };
    let (w, h) = (refdef.vrect[2], refdef.vrect[3]);
    input::set_view_size(w, h);
    gl::draw_crosshair(w, h, input::cursor());

    let n = FRAME.fetch_add(1, Ordering::Relaxed);
    if st.settings.shots.contains(&n) {
        console(st, c"screenshot\n");
    }
    if st.settings.quit_after == Some(n) {
        console(st, c"quit\n");
    }
}
