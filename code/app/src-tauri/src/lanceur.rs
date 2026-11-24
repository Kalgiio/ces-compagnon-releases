//! Bouton « RSI Launcher » (demande du CES, 21/11) : ouvre le lanceur officiel de RSI, rien de plus.
//! L'application ne lance pas le jeu elle-même et n'y touche pas : c'est le lanceur qui s'en charge, comme d'habitude.
//!
//! Recherche : à côté du dossier du jeu (`…\RSI\StarCitizen\LIVE` → `…\RSI\RSI Launcher`), puis aux emplacements
//! habituels sur chaque lecteur, puis le chemin choisi à la main (fichier `lanceur.json` des données).

use crate::traduction::Dossiers;
use std::path::{Path, PathBuf};

const EXE: &str = "RSI Launcher.exe";

fn fichier_choix(d: &Dossiers) -> PathBuf {
    d.donnees.join("lanceur.json")
}

fn est_lanceur(p: &Path) -> bool {
    p.is_file() && p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(EXE))
}

/// Candidats tirés du dossier du jeu : on remonte jusqu'à 3 niveaux et on cherche `RSI Launcher\RSI Launcher.exe`.
fn depuis_le_jeu(dossier_jeu: &Path) -> Option<PathBuf> {
    dossier_jeu.ancestors().skip(1).take(3).map(|a| a.join("RSI Launcher").join(EXE)).find(|p| est_lanceur(p))
}

fn emplacements_habituels() -> Option<PathBuf> {
    const SOUS_CHEMINS: &[&str] = &[
        r"Program Files\Roberts Space Industries\RSI Launcher",
        r"RSI\RSI Launcher",
        r"Roberts Space Industries\RSI Launcher",
        r"Games\RSI\RSI Launcher",
        r"Games\Roberts Space Industries\RSI Launcher",
    ];
    (b'C'..=b'Z')
        .map(|l| format!("{}:\\", l as char))
        .filter(|r| Path::new(r).exists())
        .flat_map(|r| SOUS_CHEMINS.iter().map(move |s| Path::new(&r).join(s).join(EXE)))
        .find(|p| est_lanceur(p))
}

fn choisi(d: &Dossiers) -> Option<PathBuf> {
    let t = std::fs::read_to_string(fichier_choix(d)).ok()?;
    let v: serde_json::Value = serde_json::from_str(&t).ok()?;
    Some(PathBuf::from(v["chemin"].as_str()?)).filter(|p| est_lanceur(p))
}

pub fn trouver(dossier_jeu: Option<&Path>, d: &Dossiers) -> Option<PathBuf> {
    choisi(d).or_else(|| dossier_jeu.and_then(depuis_le_jeu)).or_else(emplacements_habituels)
}

fn dossier_jeu(surveillance: &crate::jeu::Surveillance) -> Option<PathBuf> {
    surveillance.etat.lock().unwrap().dossier_jeu.clone().map(PathBuf::from)
}

/// Chemin du lanceur trouvé, ou `None`.
#[tauri::command(async)]
pub fn rsi_lanceur_etat(surveillance: tauri::State<'_, crate::jeu::Surveillance>, dossiers: tauri::State<'_, Dossiers>) -> Option<String> {
    trouver(dossier_jeu(&surveillance).as_deref(), &dossiers).map(|p| p.display().to_string())
}

/// Ouvre le RSI Launcher (processus indépendant de l'application).
#[tauri::command(async)]
pub fn rsi_lanceur_ouvrir(surveillance: tauri::State<'_, crate::jeu::Surveillance>, dossiers: tauri::State<'_, Dossiers>) -> Result<(), String> {
    let exe = trouver(dossier_jeu(&surveillance).as_deref(), &dossiers)
        .ok_or("RSI Launcher introuvable : indique son emplacement avec « Choisir… ».")?;
    let mut cmd = std::process::Command::new(&exe);
    if let Some(dossier) = exe.parent() {
        cmd.current_dir(dossier);
    }
    cmd.spawn().map(|_| ()).map_err(|e| format!("Impossible d'ouvrir le RSI Launcher : {e}"))
}

/// Choix manuel de `RSI Launcher.exe` (vérifié par son nom), mémorisé.
#[tauri::command(async)]
pub fn rsi_lanceur_choisir(app: tauri::AppHandle, dossiers: tauri::State<'_, Dossiers>) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let Some(choix) = app.dialog().file().set_title("Emplacement de RSI Launcher.exe").add_filter("RSI Launcher", &["exe"]).blocking_pick_file()
    else {
        return Ok(None);
    };
    let p = choix.into_path().map_err(|e| e.to_string())?;
    if !est_lanceur(&p) {
        return Err(format!("« {} » n'est pas « {EXE} ».", p.display()));
    }
    crate::traduction::ecrire_sur(&fichier_choix(&dossiers), serde_json::json!({ "chemin": p.display().to_string() }).to_string().as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(Some(p.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trouve_a_cote_du_jeu_et_choix_manuel() {
        let base = std::env::temp_dir().join(format!("cmp-lanceur-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let jeu = base.join("RSI").join("StarCitizen").join("LIVE");
        std::fs::create_dir_all(&jeu).unwrap();
        let d = Dossiers { donnees: base.join("donnees") };
        assert_eq!(depuis_le_jeu(&jeu), None);
        let exe = base.join("RSI").join("RSI Launcher").join(EXE);
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"").unwrap();
        assert_eq!(trouver(Some(&jeu), &d), Some(exe.clone()));
        // Un choix manuel qui n'est pas le lanceur est ignoré.
        crate::traduction::ecrire_sur(&fichier_choix(&d), br#"{"chemin":"C:\\Windows\\notepad.exe"}"#).unwrap();
        assert_eq!(choisi(&d), None);
        let _ = std::fs::remove_dir_all(&base);
    }
}
