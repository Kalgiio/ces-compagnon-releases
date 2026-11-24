//! Canal de mises à jour (docs/companion/CANAL-STABLE.md, décisions du CES) : `stable` (membres) ou `canary`
//! (version de test de le CES).
//!
//! - Canal par défaut FIXÉ À LA CONSTRUCTION : `CES_CANAL=stable|canary` (défaut `canary`).
//! - Réglage caché (testeurs) : un choix enregistré dans `%APPDATA%\…\canal.json` remplace le canal par défaut.
//! - L'updater est appelé ici avec l'adresse du canal en vigueur ; il ne propose qu'une version PLUS RÉCENTE que
//!   celle installée (jamais de rétrogradation).

use crate::traduction::{ecrire_sur, Dossiers};
use serde::Serialize;
use tauri::Manager;
use tauri_plugin_updater::UpdaterExt;

const DEPOT: &str = "https://github.com/Kalgiio/ces-compagnon-releases/releases/download";

/// Canal choisi à la construction.
pub fn canal_build() -> &'static str {
    match option_env!("CES_CANAL") {
        Some("stable") => "stable",
        _ => "canary",
    }
}

fn manifeste(canal: &str) -> String {
    match canal {
        "stable" => format!("{DEPOT}/stable-latest/latest.json"),
        _ => format!("{DEPOT}/canary/latest.json"),
    }
}

fn fichier(d: &Dossiers) -> std::path::PathBuf {
    d.donnees.join("canal.json")
}

/// Canal en vigueur : choix local s'il existe, sinon celui de la construction.
pub fn canal_actif(d: &Dossiers) -> String {
    std::fs::read(fichier(d))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v["canal"].as_str().map(str::to_string))
        .filter(|c| c == "stable" || c == "canary")
        .unwrap_or_else(|| canal_build().to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EtatCanal {
    build: &'static str,
    actif: String,
    manifeste: String,
}

#[tauri::command]
pub fn canal_etat(dossiers: tauri::State<'_, Dossiers>) -> EtatCanal {
    let actif = canal_actif(&dossiers);
    EtatCanal { build: canal_build(), manifeste: manifeste(&actif), actif }
}

/// Réglage caché des testeurs. `None` = revenir au canal de la construction.
#[tauri::command]
pub fn canal_choisir(dossiers: tauri::State<'_, Dossiers>, canal: Option<String>) -> Result<EtatCanal, String> {
    match canal.as_deref() {
        Some(c @ ("stable" | "canary")) => {
            ecrire_sur(&fichier(&dossiers), serde_json::json!({ "canal": c }).to_string().as_bytes()).map_err(|e| e.to_string())?
        }
        None => {
            let _ = std::fs::remove_file(fichier(&dossiers));
        }
        Some(_) => return Err("Canal inconnu.".into()),
    }
    Ok(canal_etat(dossiers))
}

#[derive(Serialize)]
pub struct MiseAJour {
    version: String,
    notes: Option<String>,
}

fn updater(app: &tauri::AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    let canal = canal_actif(&app.state::<Dossiers>());
    let url = tauri::Url::parse(&manifeste(&canal)).map_err(|e| e.to_string())?;
    app.updater_builder().endpoints(vec![url]).map_err(|e| e.to_string())?.build().map_err(|e| e.to_string())
}

/// Vérifie une mise à jour sur le canal en vigueur ; `None` s'il n'y en a pas.
#[tauri::command]
pub async fn maj_verifier(app: tauri::AppHandle) -> Result<Option<MiseAJour>, String> {
    let u = updater(&app)?.check().await.map_err(|e| e.to_string())?;
    Ok(u.map(|u| MiseAJour { version: u.version.clone(), notes: u.body.clone() }))
}

/// Télécharge (paquet signé, vérifié par l'updater), installe, puis relance l'application. La progression est envoyée
/// à la fenêtre de mise à jour par `progression` : `{ recu, total }` pendant le téléchargement, puis `{ etape: "installation" }`.
#[tauri::command]
pub async fn maj_installer(app: tauri::AppHandle, progression: tauri::ipc::Channel<serde_json::Value>) -> Result<(), String> {
    let Some(u) = updater(&app)?.check().await.map_err(|e| e.to_string())? else {
        return Err("Aucune mise à jour disponible.".into());
    };
    let mut recu: u64 = 0;
    let p2 = progression.clone();
    u.download_and_install(
        move |n, total| {
            recu += n as u64;
            let _ = progression.send(serde_json::json!({ "recu": recu, "total": total }));
        },
        move || {
            let _ = p2.send(serde_json::json!({ "etape": "installation" }));
        },
    )
    .await
    .map_err(|e| e.to_string())?;
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canal_par_defaut_et_choix_local() {
        let d = Dossiers { donnees: std::env::temp_dir().join(format!("cmp-canal-{}", std::process::id())) };
        let _ = std::fs::remove_dir_all(&d.donnees);
        assert_eq!(canal_actif(&d), canal_build());
        ecrire_sur(&fichier(&d), br#"{"canal":"stable"}"#).unwrap();
        assert_eq!(canal_actif(&d), "stable");
        assert!(manifeste("stable").ends_with("/stable-latest/latest.json"));
        assert!(manifeste("canary").ends_with("/canary/latest.json"));
        ecrire_sur(&fichier(&d), br#"{"canal":"pirate"}"#).unwrap();
        assert_eq!(canal_actif(&d), canal_build(), "valeur inconnue ignorée");
    }
}
