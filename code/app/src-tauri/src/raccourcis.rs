//! SOS au clavier (demande du CES, 24/11) : lancer un SOS sans quitter le jeu, avec une touche par type.
//!
//! Raccourcis GLOBAUX de Windows (RegisterHotKey, comme Discord ou OBS) : l'application ne lit rien dans le jeu et n'y
//! injecte rien. Désactivés par défaut (Réglages › Notifications). Par défaut Ctrl + Maj + 1…6 = combat, panne,
//! carburant, médical, transport, autre ; chaque combinaison est RÉGLABLE par le membre (25/11) : F1 à F24 seules, ou
//! toute combinaison avec Ctrl, Alt ou Windows. Garde-fou contre l'appui accidentel : il faut appuyer DEUX FOIS sur la
//! même combinaison en moins de 3 s — le premier appui affiche la carte CES « appuie encore pour envoyer ».

use crate::presence::{jeton_actuel, Portail};
use std::collections::BTreeMap;
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

/// Raccourcis enregistrés : identifiant du raccourci → (type, libellé).
#[derive(Default)]
pub struct Actifs(pub Mutex<Vec<(u32, &'static str, &'static str)>>);

const FONCTIONS: [Code; 24] = [
    Code::F1, Code::F2, Code::F3, Code::F4, Code::F5, Code::F6, Code::F7, Code::F8, Code::F9, Code::F10, Code::F11, Code::F12,
    Code::F13, Code::F14, Code::F15, Code::F16, Code::F17, Code::F18, Code::F19, Code::F20, Code::F21, Code::F22, Code::F23, Code::F24,
];

fn defaut(code: Code) -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), code)
}

/// Vérifie une combinaison saisie par le membre et la remet au format canonique.
/// Acceptées : F1 à F24 (seules ou avec des modificateurs), ou une touche avec Ctrl, Alt ou Windows.
/// Refusées : une touche seule (ou avec Maj seulement) qui empêcherait d'écrire, et les combinaisons du système.
pub fn valider(texte: &str) -> Result<Shortcut, String> {
    let s: Shortcut = texte.trim().parse().map_err(|_| format!("Combinaison inconnue : « {} ».", texte.trim()))?;
    let fonction = FONCTIONS.contains(&s.key);
    let fort = s.mods.intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER);
    if !fonction && !fort {
        return Err("Ajoute Ctrl, Alt ou Windows à cette touche (seules F1 à F24 peuvent s'utiliser sans).".into());
    }
    let reserve = (s.mods == Modifiers::ALT && s.key == Code::F4)
        || (s.mods.contains(Modifiers::CONTROL | Modifiers::ALT) && s.key == Code::Delete)
        || (s.mods == Modifiers::SUPER && matches!(s.key, Code::KeyL | Code::KeyD | Code::KeyE | Code::KeyR | Code::Tab))
        || (s.mods == Modifiers::ALT && s.key == Code::Tab)
        || (s.mods == (Modifiers::CONTROL | Modifiers::SHIFT) && s.key == Code::Escape);
    if reserve {
        return Err("Cette combinaison est réservée à Windows.".into());
    }
    if matches!(s.key, Code::ShiftLeft | Code::ShiftRight | Code::ControlLeft | Code::ControlRight | Code::AltLeft | Code::AltRight | Code::MetaLeft | Code::MetaRight) {
        return Err("Termine la combinaison par une touche (pas seulement Ctrl, Alt ou Maj).".into());
    }
    Ok(s)
}

/// Texte lisible et canonique (« Ctrl+Shift+F13 »), relu tel quel par `valider`.
pub fn texte(s: &Shortcut) -> String {
    let mut p: Vec<String> = Vec::new();
    if s.mods.contains(Modifiers::CONTROL) { p.push("Ctrl".into()); }
    if s.mods.contains(Modifiers::ALT) { p.push("Alt".into()); }
    if s.mods.contains(Modifiers::SHIFT) { p.push("Shift".into()); }
    if s.mods.contains(Modifiers::SUPER) { p.push("Super".into()); }
    let k = format!("{:?}", s.key);
    p.push(k.strip_prefix("Key").or_else(|| k.strip_prefix("Digit")).unwrap_or(&k).to_string());
    p.join("+")
}

/// Combinaisons en vigueur (réglages du membre, sinon Ctrl+Maj+1…6), une par type, sans doublon.
pub fn combinaisons(perso: &BTreeMap<String, String>) -> Result<Vec<(Shortcut, &'static str, &'static str)>, String> {
    let mut v: Vec<(Shortcut, &'static str, &'static str)> = Vec::new();
    for (code, kind, libelle) in TYPES {
        let s = match perso.get(kind).filter(|t| !t.trim().is_empty()) {
            Some(t) => valider(t).map_err(|e| format!("{libelle} : {e}"))?,
            None => defaut(code),
        };
        if let Some((_, _, autre)) = v.iter().find(|(x, _, _)| x.id() == s.id()) {
            return Err(format!("{libelle} et {autre} ont la même combinaison ({}).", texte(&s)));
        }
        v.push((s, kind, libelle));
    }
    Ok(v)
}

/// Active ou coupe les raccourcis (appelé au démarrage et quand les réglages changent).
pub fn appliquer(app: &tauri::AppHandle, actif: bool, perso: &BTreeMap<String, String>) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all(); // l'application n'enregistre que ces raccourcis-là
    let actifs = app.state::<Actifs>();
    actifs.0.lock().unwrap().clear();
    if !actif {
        return Ok(());
    }
    let liste = combinaisons(perso)?;
    for (s, kind, libelle) in &liste {
        gs.register(*s).map_err(|e| format!("{libelle} : {} est déjà pris par une autre application ({e}).", texte(s)))?;
        actifs.0.lock().unwrap().push((s.id(), kind, libelle));
    }
    Ok(())
}

/// Gestionnaire du plugin : premier appui = armé (carte d'avertissement), second appui dans les 3 s = SOS envoyé.
pub fn sur_raccourci(app: &tauri::AppHandle, s: &Shortcut, etat: ShortcutState) {
    if etat != ShortcutState::Pressed {
        return;
    }
    let Some(actifs) = app.try_state::<Actifs>() else { return };
    let Some((_, kind, libelle)) = actifs.0.lock().unwrap().iter().find(|(id, _, _)| *id == s.id()).copied() else { return };
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

/// Vérifie une combinaison saisie dans Réglages → texte canonique, ou la raison du refus.
#[tauri::command]
pub fn raccourci_verifier(texte_saisi: String) -> Result<String, String> {
    valider(&texte_saisi).map(|s| texte(&s))
}

/// Pendant la saisie d'une combinaison, les raccourcis sont suspendus (sinon Windows les intercepte avant la fenêtre).
#[tauri::command]
pub fn raccourcis_suspendre(app: tauri::AppHandle, notifs: tauri::State<'_, crate::notif::Notifications>, suspendre: bool) -> Result<(), String> {
    if suspendre {
        let _ = app.global_shortcut().unregister_all();
        app.state::<Actifs>().0.lock().unwrap().clear();
        Ok(())
    } else {
        let (actif, perso) = notifs.raccourcis_sos();
        appliquer(&app, actif, &perso)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_types_distincts() {
        let mut k: Vec<&str> = TYPES.iter().map(|t| t.1).collect();
        k.dedup();
        assert_eq!(k.len(), 6);
    }

    #[test]
    fn touches_de_fonction_et_combinaisons() {
        assert_eq!(texte(&valider("F13").unwrap()), "F13");
        assert_eq!(texte(&valider("F24").unwrap()), "F24");
        assert_eq!(texte(&valider("ctrl+shift+F1").unwrap()), "Ctrl+Shift+F1");
        assert_eq!(texte(&valider("Alt+KeyK").unwrap()), "Alt+K");
        assert_eq!(texte(&valider("Ctrl+Shift+Digit1").unwrap()), "Ctrl+Shift+1");
        // relu tel quel
        assert_eq!(texte(&valider("Ctrl+Shift+1").unwrap()), "Ctrl+Shift+1");
        assert!(valider("K").is_err());
        assert!(valider("Shift+K").is_err());
        assert!(valider("Alt+F4").is_err());
        assert!(valider("Super+L").is_err());
    }

    #[test]
    fn doublons_refuses() {
        let mut p = BTreeMap::new();
        assert_eq!(combinaisons(&p).unwrap().len(), 6);
        p.insert("panne".to_string(), "Ctrl+Shift+1".to_string());
        assert!(combinaisons(&p).is_err());
        p.insert("panne".to_string(), "F14".to_string());
        assert!(combinaisons(&p).is_ok());
    }
}
