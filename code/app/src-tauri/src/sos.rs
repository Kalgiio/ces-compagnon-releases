//! SOS (demande du CES, 21/11) : « envoyer un SOS au CES, activable via l'application Windows ».
//!
//! Routes du portail, contrat dans `docs/companion/API.md` § 5 sexies, appelées par le cœur avec le jeton ; réservé à
//! la webview principale. Le direct (`sos:update`) arrive par la boucle de `notif.rs` et est relayé à l'interface par
//! l'événement `sos-evenement`. Rien ne touche au jeu : le lieu vient du dernier `location.enter` que le portail a reçu.

use crate::presence::{jeton_actuel, Portail};
use reqwest::Method;
use serde_json::{json, Value};

const TYPES: [&str; 6] = ["combat", "panne", "carburant", "medical", "transport", "autre"];

/// Identifiant d'alerte : chiffres, lettres et tirets seulement (va dans l'URL).
fn id_valide(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Texte brut borné (le portail borne aussi) ; vide → absent.
fn borne(t: Option<String>, max: usize) -> Option<String> {
    t.map(|t| t.trim().chars().take(max).collect::<String>()).filter(|t| !t.is_empty())
}

/// `etat` | `envoyer` (type, message?) | `repondre` (id) | `clore` (id, annuler?).
#[tauri::command]
pub async fn sos_action(
    portail: tauri::State<'_, Portail>,
    action: String,
    kind: Option<String>,
    message: Option<String>,
    id: Option<String>,
    annuler: Option<bool>,
) -> Result<Value, String> {
    let jeton = jeton_actuel(&portail)?;
    let alerte = || -> Result<String, String> { id.clone().filter(|i| id_valide(i)).ok_or_else(|| "Alerte inconnue.".into()) };
    let (methode, chemin, corps): (Method, String, Option<Value>) = match action.as_str() {
        "etat" => (Method::GET, "sos".into(), None),
        "envoyer" => {
            let k = kind.filter(|k| TYPES.contains(&k.as_str())).unwrap_or_else(|| "autre".into());
            (Method::POST, "sos".into(), Some(json!({ "kind": k, "message": borne(message, 500) })))
        }
        "repondre" => (Method::POST, format!("sos/{}/respond", alerte()?), None),
        "clore" => {
            let corps = if annuler == Some(true) { json!({ "cancel": true }) } else { json!({}) };
            (Method::POST, format!("sos/{}/close", alerte()?), Some(corps))
        }
        _ => return Err("Action inconnue.".into()),
    };
    tauri::async_runtime::spawn_blocking(move || crate::portail::api_json(&jeton, methode, &chemin, corps.as_ref()).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controles() {
        assert!(id_valide("0b7e2a3c-1111-4222-8333-944455556666") && id_valide("42"));
        assert!(!id_valide("../x") && !id_valide("") && !id_valide("a/b"));
        assert_eq!(borne(Some("  ".into()), 10), None);
        assert_eq!(borne(Some(" abcdef ".into()), 3).as_deref(), Some("abc"));
    }
}
