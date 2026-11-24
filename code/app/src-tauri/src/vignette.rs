//! Bouton lecture / pause dans l'aperçu de la barre des tâches (demande du CES, 24/11) : survoler l'icône de CES
//! Compagnon dans la barre des tâches montre la miniature avec un bouton pour couper la radio sans ouvrir la fenêtre.
//!
//! API Windows « thumbnail toolbar » (ITaskbarList3::ThumbBarAddButtons) ; le clic arrive en WM_COMMAND
//! (THBN_CLICKED) dans la procédure de la fenêtre, interceptée par une sous-classe (SetWindowSubclass), puis il est
//! relayé à l'interface par l'événement `radio-basculer`. L'icône suit l'état (commande `vignette_radio`).

use std::sync::OnceLock;
use tauri::{Emitter, EventTarget, Manager};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::CreateBitmap;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Shell::{DefSubclassProc, ITaskbarList3, SetWindowSubclass, TaskbarList, THBF_ENABLED, THB_FLAGS, THB_ICON, THB_TOOLTIP, THUMBBUTTON};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, RegisterWindowMessageW, HICON, ICONINFO, WM_COMMAND};

const ID_BOUTON: u32 = 4100;
const THBN_CLICKED: usize = 0x1800;
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
static FENETRE: OnceLock<isize> = OnceLock::new();
static MSG_BOUTON_CREE: OnceLock<u32> = OnceLock::new();

/// Icône 20 × 20 blanche dessinée à la main : triangle (lecture) ou deux barres (pause).
fn icone(pause: bool) -> Option<HICON> {
    const T: i32 = 20;
    let mut px = vec![0u8; (T * T * 4) as usize];
    for y in 0..T {
        for x in 0..T {
            let dedans = if pause {
                (5..=8).contains(&x) && (4..=15).contains(&y) || (11..=14).contains(&x) && (4..=15).contains(&y)
            } else {
                // triangle pointant à droite : base x = 6, sommet x = 15, centre y = 9.5
                x >= 6 && x <= 15 && ((y as f32 - 9.5).abs() <= (15 - x) as f32 * 0.62)
            };
            if dedans {
                let i = ((y * T + x) * 4) as usize;
                px[i..i + 4].copy_from_slice(&[255, 255, 255, 255]); // BGRA prémultiplié
            }
        }
    }
    let masque = vec![0u8; (T * T / 8) as usize];
    unsafe {
        let couleur = CreateBitmap(T, T, 1, 32, Some(px.as_ptr().cast()));
        let m = CreateBitmap(T, T, 1, 1, Some(masque.as_ptr().cast()));
        let ii = ICONINFO { fIcon: true.into(), xHotspot: 0, yHotspot: 0, hbmMask: m, hbmColor: couleur };
        CreateIconIndirect(&ii).ok()
    }
}

fn bouton(pause: bool) -> THUMBBUTTON {
    let mut b = THUMBBUTTON { dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS, iId: ID_BOUTON, dwFlags: THBF_ENABLED, ..Default::default() };
    if let Some(i) = icone(pause) {
        b.hIcon = i;
    }
    let texte: Vec<u16> = if pause { "Mettre la radio en pause" } else { "Écouter la radio du CES" }.encode_utf16().collect();
    b.szTip[..texte.len()].copy_from_slice(&texte);
    b
}

fn barre() -> Option<ITaskbarList3> {
    unsafe {
        let t: ITaskbarList3 = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).ok()?;
        t.HrInit().ok()?;
        Some(t)
    }
}

fn ajouter(hwnd: HWND) {
    if let Some(t) = barre() {
        unsafe {
            let _ = t.ThumbBarAddButtons(hwnd, &[bouton(false)]); // échoue sans dommage si déjà ajouté
        }
    }
}

unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _ref: usize) -> LRESULT {
    if Some(&msg) == MSG_BOUTON_CREE.get() {
        ajouter(hwnd);
    } else if msg == WM_COMMAND && (wp.0 >> 16) & 0xffff == THBN_CLICKED && (wp.0 & 0xffff) as u32 == ID_BOUTON {
        if let Some(app) = APP.get() {
            let _ = app.emit_to(EventTarget::webview("main"), "radio-basculer", ());
        }
    }
    unsafe { DefSubclassProc(hwnd, msg, wp, lp) }
}

/// À appeler une fois au démarrage (fil principal).
pub fn installer(app: &tauri::AppHandle) {
    let Some(f) = app.get_webview_window("main") else { return };
    let Ok(h) = f.hwnd() else { return };
    let brut = h.0 as isize;
    let _ = APP.set(app.clone());
    let _ = FENETRE.set(brut);
    let hwnd = HWND(brut as *mut _);
    unsafe {
        let _ = MSG_BOUTON_CREE.set(RegisterWindowMessageW(w!("TaskbarButtonCreated")));
        let _ = SetWindowSubclass(hwnd, Some(procedure), 1, 0);
    }
    // Le bouton de la barre des tâches existe peut-être déjà : on ajoute aussi tout de suite (sans effet s'il est trop tôt,
    // le message TaskbarButtonCreated rattrapera).
    ajouter(hwnd);
    let _ = PCWSTR::null();
}

/// Icône et info-bulle selon l'état de la radio.
#[tauri::command]
pub fn vignette_radio(app: tauri::AppHandle, en_lecture: bool) {
    let _ = app.run_on_main_thread(move || {
        let (Some(t), Some(&h)) = (barre(), FENETRE.get()) else { return };
        unsafe {
            let _ = t.ThumbBarUpdateButtons(HWND(h as *mut _), &[bouton(en_lecture)]);
        }
    });
}
