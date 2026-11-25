//! Conditions d'utilisation et données personnelles (demande du CES, 25/11) : texte unique
//! `installer/conditions.txt`, montré par l'installeur (page « Conditions ») ET par l'application au lancement
//! tant que la version en vigueur n'est pas acceptée — y compris pour les installations faites avant (1.0.0).
//!
//! L'acceptation est gardée sur ce PC (`conditions-acceptees.txt` dans le dossier de données) :
//! `<version>|<date ISO>|<origine>`. L'installeur l'écrit quand la page Conditions a été montrée (installation
//! normale, pas une mise à jour silencieuse) ; l'application l'écrit quand on clique « J'accepte ».

use crate::traduction::{ecrire_sur, Dossiers};
use serde::Serialize;

/// À changer à chaque modification du texte : l'application le présentera de nouveau.
pub const VERSION: &str = "2026-11-25";
const FICHIER: &str = "conditions-acceptees.txt";

#[derive(Serialize)]
pub struct Etat {
    version: &'static str,
    acceptee: bool,
    le: Option<String>,
}

fn lire(d: &Dossiers) -> Option<(String, String)> {
    let s = std::fs::read_to_string(d.donnees.join(FICHIER)).ok()?;
    let mut p = s.trim().trim_start_matches('\u{feff}').split('|');
    Some((p.next()?.trim().to_string(), p.next().unwrap_or("").trim().to_string()))
}

#[tauri::command]
pub fn conditions_etat(dossiers: tauri::State<'_, Dossiers>) -> Etat {
    match lire(&dossiers) {
        Some((v, le)) if v == VERSION => Etat { version: VERSION, acceptee: true, le: Some(le).filter(|l| !l.is_empty()) },
        _ => Etat { version: VERSION, acceptee: false, le: None },
    }
}

#[tauri::command]
pub fn conditions_accepter(dossiers: tauri::State<'_, Dossiers>) -> Result<Etat, String> {
    let le = chrono_iso();
    ecrire_sur(&dossiers.donnees.join(FICHIER), format!("{VERSION}|{le}|application\n").as_bytes()).map_err(|e| e.to_string())?;
    Ok(Etat { version: VERSION, acceptee: true, le: Some(le) })
}

/// Ouvre un lien du texte des conditions dans le navigateur : SEULEMENT les adresses écrites dans ce texte.
#[tauri::command]
pub fn conditions_ouvrir_lien(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let texte = include_str!("../installer/conditions.txt");
    let connu = texte.split(|c: char| c.is_whitespace() || c == ')' || c == '(').any(|m| m == url);
    if !connu || !url.starts_with("https://") {
        return Err("Lien inconnu.".into());
    }
    app.opener().open_url(&url, None::<&str>).map_err(|e| e.to_string())
}

/// Date UTC ISO 8601 sans dépendance (secondes depuis 1970 → calendrier civil).
fn chrono_iso() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let (jours, s) = (t.div_euclid(86400), t.rem_euclid(86400));
    let z = jours + 719_468;
    let ere = z.div_euclid(146_097);
    let doe = z - ere * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let a = yoe + ere * 400 + if m <= 2 { 1 } else { 0 };
    format!("{a:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", s / 3600, (s % 3600) / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_iso() {
        let d = chrono_iso();
        assert_eq!(d.len(), 20);
        assert!(d.starts_with("20"));
    }

    #[test]
    fn version_du_texte() {
        let t = include_str!("../installer/conditions.txt");
        assert!(t.contains("Version du 25 novembre 2026"), "VERSION et la date du texte doivent aller ensemble");
    }

    #[test]
    fn installeur_meme_version() {
        assert!(include_str!("../installer/hooks.nsh").contains(&format!("\"{VERSION}|installeur")), "hooks.nsh doit écrire la même VERSION");
    }
}
