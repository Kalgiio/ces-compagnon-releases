//! Carte CES sur le bureau (demande du CES, 22/11, d'après sa maquette) : quand un membre lance Star Citizen (ou lance
//! un SOS), une petite carte aux couleurs du CES apparaît en bas à droite de l'écran, au-dessus des autres fenêtres,
//! puis disparaît seule. Elle remplace la notification Windows pour ces deux cas (réglage `carte`).
//!
//! Fenêtre `annonce` sans cadre, transparente, toujours au premier plan, absente de la barre des tâches, qui ne prend
//! pas le focus (le jeu ou l'application en cours le garde). Elle n'appelle aucune commande : son contenu arrive dans
//! l'adresse (`annonce.html#…`), elle est fermée par le cœur après quelques secondes.

use base64::Engine;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "annonce";
const LARGEUR: f64 = 380.0;
const HAUTEUR: f64 = 96.0;
const MARGE: f64 = 16.0;
const DUREE: Duration = Duration::from_secs(7);
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// `ton` : `lancement` | `sos`. `image` : avatar en cache (facultatif), envoyé en `data:` URL.
pub fn montrer(app: &tauri::AppHandle, nom: &str, texte: &str, ton: &str, image: Option<&Path>) {
    let image = image
        .and_then(|p| std::fs::read(p).ok())
        .filter(|o| o.len() < 200_000)
        .map(|o| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(o)));
    let donnees = serde_json::json!({ "titre": nom, "nom": nom, "texte": texte, "ton": ton, "image": image });
    let fragment = urlencoding(&donnees.to_string());
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let app2 = app.clone();
    // Création sur le fil principal (exigence de Windows pour les fenêtres).
    let _ = app.run_on_main_thread(move || {
        if let Some(ancienne) = app2.get_webview_window(LABEL) {
            let _ = ancienne.destroy();
        }
        let Some(ecran) = app2.primary_monitor().ok().flatten() else { return };
        let echelle = ecran.scale_factor();
        let zone = ecran.work_area();
        let (l, h) = ((LARGEUR * echelle) as i32, (HAUTEUR * echelle) as i32);
        let marge = (MARGE * echelle) as i32;
        let x = zone.position.x + zone.size.width as i32 - l - marge;
        let y = zone.position.y + zone.size.height as i32 - h - marge;
        let construite = WebviewWindowBuilder::new(&app2, LABEL, WebviewUrl::App(format!("annonce.html#{fragment}").into()))
            .title("CES Compagnon")
            .inner_size(LARGEUR, HAUTEUR)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(false)
            .visible(false)
            .build();
        if let Ok(w) = construite {
            let _ = w.set_position(PhysicalPosition::new(x, y));
            let _ = w.show();
        }
    });
    let app3 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(DUREE);
        if GENERATION.load(Ordering::SeqCst) == generation {
            let _ = app3.run_on_main_thread({
                let app4 = app3.clone();
                move || {
                    if let Some(w) = app4.get_webview_window(LABEL) {
                        let _ = w.destroy();
                    }
                }
            });
        }
    });
}

/// Encodage pour un fragment d'URL (tout sauf lettres, chiffres et `-_.~`).
fn urlencoding(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn encodage() {
        assert_eq!(super::urlencoding("a é#"), "a%20%C3%A9%23");
    }
}
