//! SOS au clavier (demande du CES, 24/11) : lancer un SOS sans quitter le jeu, avec une touche par type.
//!
//! Raccourcis GLOBAUX de Windows (RegisterHotKey, comme Discord ou OBS) : l'application ne lit rien dans le jeu et n'y
//! injecte rien. Désactivés par défaut (Réglages › Notifications). Ctrl + Maj + 1…6 = combat, panne, carburant,
//! médical, transport, autre. Garde-fou contre l'appui accidentel : il faut appuyer DEUX FOIS sur la même combinaison
//! en moins de 3 s — le premier appui affiche la carte CES « appuie encore pour envoyer », le second envoie.

use crate::presence::{jeton_actuel, Portail};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub const TYPES: [(Code, &str, &str); 6] = [
    (Code::Digit1, "combat", "Combat"),
    (Code::Digit2, "panne", "Panne"),
    (Code::Digit3, "carburant", "Carburant"),
    (Code::Digit4, "medical", "Médical"),
    (Code::Digit5, "transport", "Transport"),
    (Code::Digit6, "autre", "Autre"),
];
const DELAI: Duration = Duration::from_secs(3);

#[derive(Default)]
pub struct Attente(pub Mutex<Option<(&'static str, Instant)>>);

fn raccourci(code: Code) -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), code)
}

/// Active ou coupe les raccourcis (appelé au démarrage et quand le réglage change).
pub fn appliquer(app: &tauri::AppHandle, actif: bool) -> Result<(), String> {
    let gs = app.global_shortcut();
    for (code, _, _) in TYPES {
        let r = raccourci(code);
        if gs.is_registered(r) {
            let _ = gs.unregister(r);
        }
        if actif {
            gs.register(r).map_err(|e| format!("Raccourci Ctrl+Maj+{} déjà pris par une autre application : {e}", code_vers_chiffre(code)))?;
        }
    }
    Ok(())
}

fn code_vers_chiffre(code: Code) -> u8 {
    TYPES.iter().position(|(c, _, _)| *c == code).map_or(0, |i| i as u8 + 1)
}

/// Gestionnaire du plugin : premier appui = armé (carte d'avertissement), second appui dans les 3 s = SOS envoyé.
pub fn sur_raccourci(app: &tauri::AppHandle, s: &Shortcut, etat: ShortcutState) {
    if etat != ShortcutState::Pressed || s.mods != (Modifiers::CONTROL | Modifiers::SHIFT) {
        return;
    }
    let Some((_, kind, libelle)) = TYPES.iter().find(|(c, _, _)| *c == s.key).copied() else { return };
    let Some(attente) = app.try_state::<Attente>() else { return };
    let confirme = {
        let mut a = attente.0.lock().unwrap();
        let ok = matches!(*a, Some((k, t)) if k == kind && t.elapsed() < DELAI);
        *a = if ok { None } else { Some((kind, Instant::now())) };
        ok
    };
    if !confirme {
        crate::annonce::montrer(app, &format!("SOS {libelle} ?"), "Appuie encore sur la même touche pour l'envoyer au CES", "sos", None);
        return;
    }
    let app2 = app.clone();
    std::thread::spawn(move || {
        let resultat = app2
            .try_state::<Portail>()
            .ok_or_else(|| "Application pas prête.".to_string())
            .and_then(|p| jeton_actuel(&p))
            .and_then(|jeton| {
                crate::portail::api_json(&jeton, reqwest::Method::POST, "sos", Some(&serde_json::json!({ "kind": kind }))).map_err(|e| e.to_string())
            });
        match resultat {
            Ok(_) => crate::annonce::montrer(&app2, &format!("SOS {libelle} envoyé"), "Le CES est prévenu (portail, Discord, application)", "sos", None),
            Err(e) => crate::annonce::montrer(&app2, "SOS non envoyé", &e, "sos", None),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_types_distincts() {
        let mut k: Vec<&str> = TYPES.iter().map(|t| t.1).collect();
        k.dedup();
        assert_eq!(k.len(), 6);
        assert_eq!(code_vers_chiffre(Code::Digit4), 4);
    }
}
