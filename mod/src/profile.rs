//! Adresses des fonctions et variables de QuakeSpasm utilisées par le mod.
//!
//! D'abord cherchées par nom dans la table des symboles de l'exécutable ; à
//! défaut, on se rabat sur les offsets trouvés par rétro-ingénierie (REA +
//! Ghidra) sur le binaire strippé de référence, reconnu à sa signature.

use std::collections::HashMap;

use crate::module::MainModule;

macro_rules! addresses {
    ($( $(#[$doc:meta])* $field:ident = $symbol:literal @ $offset:literal; )*) => {
        /// Adresses absolues dans le processus.
        pub struct Addresses {
            $( $(#[$doc])* pub $field: usize, )*
        }

        impl Addresses {
            fn from_symbols(base: usize, symbols: &HashMap<String, u64>) -> Option<Addresses> {
                Some(Addresses { $( $field: base + *symbols.get($symbol)? as usize, )* })
            }

            fn from_known_build(base: usize) -> Addresses {
                Addresses { $( $field: base + $offset, )* }
            }
        }
    };
}

// Chemins de découverte dans le binaire strippé (adresses Ghidra = offset + 0x100000).
// Une variable de console (`cvar_t`) commence par un pointeur vers son nom :
// la relocation R_X86_64_RELATIVE qui vise la chaîne du nom donne son adresse.
addresses! {
    /// `void GL_SetFrustum(float, float)` : unique appelant de l'import glFrustum.
    gl_set_frustum = "GL_SetFrustum" @ 0xe740;
    /// `void R_SetFrustum(float, float)` : lit `r_stereo.value` (cvar "r_stereo").
    /// Écrit `frustum`, lit `r_origin`, `vpn`, `vright`, `vup`.
    r_set_frustum = "R_SetFrustum" @ 0xe5b0;
    frustum = "frustum" @ 0xb7000;
    r_origin = "r_origin" @ 0xb6fa8;
    vpn = "vpn" @ 0xb6fc8;
    vright = "vright" @ 0xb6fb8;
    vup = "vup" @ 0xb6fd8;
    /// Dans R_SetupView (unique appelant de R_SetFrustum) : r_origin est copié
    /// depuis r_refdef.vieworg (+120), puis AngleVectors(r_refdef.viewangles…).
    r_refdef = "r_refdef" @ 0xb6f00;
    angle_vectors = "AngleVectors" @ 0x755e0;
    /// `void V_RenderView(void)` : appelant de R_RenderView (qui lit r_stereo).
    v_render_view = "V_RenderView" @ 0x62170;
    /// `void CL_BaseMove(usercmd_t*)` : lit `cl_upspeed.value`.
    cl_base_move = "CL_BaseMove" @ 0x4dad0;
    /// `client_state_t cl` : CL_SendMove ("CL_SendMove: lost server connection")
    /// incrémente cl.movemessages (+0) et copie la commande dans cl.cmd (+4).
    cl = "cl" @ 0x4944c0;
    /// `void Chase_UpdateForDrawing(void)` : lit `chase_back.value`.
    chase_update = "Chase_UpdateForDrawing" @ 0x4b2c0;
    /// `void Cbuf_AddText(const char*)` : chaîne "Cbuf_AddText: overflow".
    cbuf_add_text = "Cbuf_AddText" @ 0x631c0;
    /// Variables de console modifiées par le mod.
    chase_active = "chase_active" @ 0xb3c40;
    r_drawviewmodel = "r_drawviewmodel" @ 0xb2160;
    crosshair = "crosshair" @ 0xb5600;
    sensitivity = "sensitivity" @ 0xb4140;
    sv_aim = "sv_aim" @ 0xb6120;
}

/// Début de R_SetFrustum dans le binaire de référence (indépendant de l'ASLR).
const KNOWN_BUILD_SIGNATURE: &[u8] = &[
    0x48, 0x83, 0xec, 0x18, 0x0f, 0x28, 0xd0, 0x66, 0x0f, 0xef, 0xc0, 0x0f, 0x2e, 0x05, 0xf2, 0x36,
];

impl Addresses {
    /// `None` si l'exécutable n'est pas QuakeSpasm (ou une version inconnue).
    pub fn resolve(module: &MainModule) -> Option<(Addresses, &'static str)> {
        let base = module.base;
        if let Some(addr) = Addresses::from_symbols(base, &crate::symbols::load_own())
            && module.is_code(addr.r_set_frustum, 16)
        {
            return Some((addr, "symboles de l'exécutable"));
        }
        let known = Addresses::from_known_build(base);
        module
            .code_matches(known.r_set_frustum, KNOWN_BUILD_SIGNATURE)
            .then_some((known, "profil REA du binaire de référence"))
    }
}
