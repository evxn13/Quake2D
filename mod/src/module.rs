//! Exécutable principal : adresse de chargement et segments de code.

use core::ffi::{c_int, c_void};

/// L'exécutable principal tel que chargé en mémoire.
pub struct MainModule {
    pub base: usize,
    /// Plages [début, fin) des segments exécutables.
    exec_ranges: Vec<(usize, usize)>,
}

impl MainModule {
    /// Premier module de `dl_iterate_phdr` = l'exécutable principal.
    pub fn find() -> Option<MainModule> {
        unsafe extern "C" fn first(info: *mut libc::dl_phdr_info, _size: usize, out: *mut c_void) -> c_int {
            let info = unsafe { &*info };
            let module = unsafe { &mut *(out as *mut MainModule) };
            module.base = info.dlpi_addr as usize;
            let phdrs = unsafe { core::slice::from_raw_parts(info.dlpi_phdr, info.dlpi_phnum as usize) };
            for ph in phdrs {
                if ph.p_type == libc::PT_LOAD && ph.p_flags & libc::PF_X != 0 {
                    let start = module.base + ph.p_vaddr as usize;
                    module.exec_ranges.push((start, start + ph.p_memsz as usize));
                }
            }
            1 // on s'arrête au premier
        }
        let mut module = MainModule { base: 0, exec_ranges: Vec::new() };
        unsafe { libc::dl_iterate_phdr(Some(first), &mut module as *mut MainModule as *mut c_void) };
        // Un PIE n'est jamais chargé à 0.
        (module.base != 0).then_some(module)
    }

    /// `[addr, addr + len)` est-il entièrement dans un segment exécutable ?
    pub fn is_code(&self, addr: usize, len: usize) -> bool {
        self.exec_ranges.iter().any(|&(s, e)| addr >= s && addr + len <= e)
    }
}

const PATCH_LEN: usize = 12;

/// Détour par saut absolu (`movabs rax, dest ; jmp rax`) écrit au début d'une
/// fonction. Pour appeler l'original, on remet ses octets le temps de l'appel :
/// sans danger tant que le jeu n'appelle pas la fonction depuis un autre thread.
pub struct JumpHook {
    target: usize,
    original: [u8; PATCH_LEN],
    patch: [u8; PATCH_LEN],
}

impl JumpHook {
    pub unsafe fn install(module: &MainModule, target: usize, dest: usize) -> Result<JumpHook, String> {
        if !module.is_code(target, PATCH_LEN) {
            return Err(format!("{target:#x} n'est pas dans le code de l'exécutable"));
        }
        let mut patch = [0u8; PATCH_LEN];
        patch[..2].copy_from_slice(&[0x48, 0xb8]);
        patch[2..10].copy_from_slice(&(dest as u64).to_le_bytes());
        patch[10..].copy_from_slice(&[0xff, 0xe0]);
        let mut original = [0u8; PATCH_LEN];
        unsafe {
            core::ptr::copy_nonoverlapping(target as *const u8, original.as_mut_ptr(), PATCH_LEN);
            // La page reste inscriptible : `call_original` réécrit les octets à chaque appel.
            let page = target & !0xfff;
            let len = (target + PATCH_LEN - page).next_multiple_of(0x1000);
            if libc::mprotect(page as *mut c_void, len, libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC) != 0 {
                return Err(format!("mprotect : {}", std::io::Error::last_os_error()));
            }
        }
        let hook = JumpHook { target, original, patch };
        unsafe { hook.write(&hook.patch) };
        Ok(hook)
    }

    unsafe fn write(&self, bytes: &[u8; PATCH_LEN]) {
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), self.target as *mut u8, PATCH_LEN) };
    }

    /// Exécute `f` avec la fonction d'origine restaurée (`f` reçoit son adresse).
    pub unsafe fn call_original<R>(&self, f: impl FnOnce(usize) -> R) -> R {
        unsafe { self.write(&self.original) };
        let r = f(self.target);
        unsafe { self.write(&self.patch) };
        r
    }
}
