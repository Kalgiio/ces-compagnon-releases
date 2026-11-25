//! Personnages (demande du CES, 21/11) : sauvegarde des apparences créées dans le jeu entre ce PC et le portail, avec
//! une photo du visage. Le jeu les écrit dans `LIVE\user\client\0\customcharacters\*.chf` ; l'application ne fait que
//! lire et copier ces fichiers (jamais pendant que le jeu tourne pour l'écriture), rien d'autre dans le jeu.
//!
//! Routes du portail : `/api/companion/characters` (contrat dans docs/companion/API.md).

use crate::presence::{jeton_actuel, Portail};
use crate::portail::{client, ORIGINE};
use serde::Serialize;
use serde_json::Value;
use sha1::Digest;
use std::path::{Path, PathBuf};

const MAX_CHF: u64 = 64 * 1024;
const MAX_PHOTO: u64 = 8 * 1024 * 1024;

fn dossier_personnages(jeu: &Path) -> PathBuf {
    jeu.join("user").join("client").join("0").join("customcharacters")
}

/// Nom de fichier sûr : lettres, chiffres, tiret, espace, soulignement ; se termine par `.chf`.
fn nom_sur(nom: &str) -> Option<String> {
    let base = nom.trim().trim_end_matches(".chf").trim();
    let propre: String = base.chars().filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' ')).take(60).collect();
    let propre = propre.trim().to_string();
    (!propre.is_empty()).then(|| format!("{propre}.chf"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Local {
    nom: String,
    fichier: String,
    taille: u64,
    modifie: Option<String>,
    empreinte: String,
}

/// Empreinte SHA-1 du contenu : sert à reconnaître localement deux fichiers identiques (pas de sécurité en jeu).
fn empreinte(octets: &[u8]) -> String {
    let mut h = sha1::Sha1::new();
    h.update(octets);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn lister(jeu: &Path) -> Vec<Local> {
    let mut v: Vec<Local> = std::fs::read_dir(dossier_personnages(jeu))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("chf")))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            if meta.len() > MAX_CHF {
                return None;
            }
            let octets = std::fs::read(e.path()).ok()?;
            let fichier = e.file_name().to_string_lossy().to_string();
            Some(Local {
                nom: fichier.trim_end_matches(".chf").to_string(),
                fichier,
                taille: meta.len(),
                modifie: meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| crate::jeu::iso_depuis(d.as_secs() as i64, 0)),
                empreinte: empreinte(&octets),
            })
        })
        .collect();
    v.sort_by(|a, b| a.nom.to_lowercase().cmp(&b.nom.to_lowercase()));
    v
}

fn dossier_jeu(s: &crate::jeu::Surveillance) -> Result<PathBuf, String> {
    s.etat.lock().unwrap().dossier_jeu.clone().map(PathBuf::from).ok_or_else(|| "Dossier du jeu introuvable.".into())
}

/// Personnages présents sur ce PC.
#[tauri::command(async)]
pub fn personnages_locaux(surveillance: tauri::State<'_, crate::jeu::Surveillance>) -> Result<Vec<Local>, String> {
    Ok(lister(&dossier_jeu(&surveillance)?))
}

pub(crate) fn erreur_http(r: reqwest::blocking::Response) -> String {
    let statut = r.status().as_u16();
    let v: Value = r.json().unwrap_or(Value::Null);
    v["message"].as_str().or_else(|| v["error"].as_str()).map(str::to_string).unwrap_or_else(|| format!("Erreur du portail ({statut})"))
}

/// Personnages sauvegardés sur le portail.
#[tauri::command(async)]
pub fn personnages_portail(portail: tauri::State<'_, Portail>) -> Result<Value, String> {
    let jeton = jeton_actuel(&portail)?;
    crate::portail::api_json(&jeton, reqwest::Method::GET, "companion/characters", None).map_err(|e| e.to_string())
}

/// Choisir une photo du visage (boîte de dialogue) → chemin, ou `None`.
#[tauri::command(async)]
pub fn personnage_choisir_photo(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let f = app.dialog().file().set_title("Photo du visage du personnage").add_filter("Images", &["jpg", "jpeg", "png", "webp"]).blocking_pick_file()?;
    f.into_path().ok().map(crate::choix::memoriser) // jeton opaque, jamais le chemin
}

/// `jeton` = jeton remis par une boîte de dialogue (`choix.rs`), jamais un chemin venu de l'interface.
pub(crate) fn partie_photo(jeton: &str) -> Result<reqwest::blocking::multipart::Part, String> {
    let chemin = crate::choix::chemin(jeton)?;
    let p = chemin.as_path();
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        _ => return Err("Photo : JPEG, PNG ou WebP.".into()),
    };
    if std::fs::metadata(p).map(|m| m.len()).unwrap_or(u64::MAX) > MAX_PHOTO {
        return Err("Photo trop lourde (8 Mo au plus).".into());
    }
    let octets = std::fs::read(p).map_err(|e| e.to_string())?;
    reqwest::blocking::multipart::Part::bytes(octets).file_name(format!("visage.{ext}")).mime_str(mime).map_err(|e| e.to_string())
}

/// Envoie un personnage du PC au portail, avec une photo facultative.
#[tauri::command(async)]
pub fn personnage_envoyer(
    surveillance: tauri::State<'_, crate::jeu::Surveillance>,
    portail: tauri::State<'_, Portail>,
    fichier: String,
    nom: Option<String>,
    photo: Option<String>,
) -> Result<Value, String> {
    let jeton = jeton_actuel(&portail)?;
    let nom_fichier = nom_sur(&fichier).ok_or("Fichier invalide.")?;
    let chemin = dossier_personnages(&dossier_jeu(&surveillance)?).join(&nom_fichier);
    let octets = std::fs::read(&chemin).map_err(|_| "Personnage introuvable sur ce PC.".to_string())?;
    if octets.len() as u64 > MAX_CHF {
        return Err("Fichier de personnage trop lourd.".into());
    }
    let mut form = reqwest::blocking::multipart::Form::new()
        .text("name", nom.unwrap_or_else(|| nom_fichier.trim_end_matches(".chf").to_string()).chars().take(60).collect::<String>())
        .part("chf", reqwest::blocking::multipart::Part::bytes(octets).file_name(nom_fichier).mime_str("application/octet-stream").map_err(|e| e.to_string())?);
    if let Some(p) = photo.filter(|p| !p.is_empty()) {
        form = form.part("photo", partie_photo(&p)?);
    }
    let r = client().post(format!("{ORIGINE}/api/companion/characters")).bearer_auth(&jeton).multipart(form).send().map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(erreur_http(r));
    }
    r.json().map_err(|e| e.to_string())
}

/// Ajoute ou remplace la photo d'un personnage du portail.
#[tauri::command(async)]
pub fn personnage_photo(portail: tauri::State<'_, Portail>, id: String, photo: String) -> Result<Value, String> {
    let jeton = jeton_actuel(&portail)?;
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') || id.is_empty() {
        return Err("Personnage inconnu.".into());
    }
    let form = reqwest::blocking::multipart::Form::new().part("photo", partie_photo(&photo)?);
    let r = client().put(format!("{ORIGINE}/api/companion/characters/{id}")).bearer_auth(&jeton).multipart(form).send().map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(erreur_http(r));
    }
    r.json().map_err(|e| e.to_string())
}

/// Récupère un personnage du portail dans le dossier du jeu (l'ancien fichier du même nom est sauvegardé avant).
#[tauri::command(async)]
pub fn personnage_recuperer(
    surveillance: tauri::State<'_, crate::jeu::Surveillance>,
    portail: tauri::State<'_, Portail>,
    dossiers: tauri::State<'_, crate::traduction::Dossiers>,
    id: String,
    nom: String,
) -> Result<String, String> {
    let jeu = crate::traduction::verifier_jeu_ferme(&surveillance, "ajouter un personnage")?;
    let jeton = jeton_actuel(&portail)?;
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') || id.is_empty() {
        return Err("Personnage inconnu.".into());
    }
    let nom_fichier = nom_sur(&nom).ok_or("Nom invalide.")?;
    let r = client().get(format!("{ORIGINE}/api/companion/characters/{id}/file")).bearer_auth(&jeton).send().map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(erreur_http(r));
    }
    let octets = r.bytes().map_err(|e| e.to_string())?;
    if octets.is_empty() || octets.len() as u64 > MAX_CHF {
        return Err("Fichier reçu invalide.".into());
    }
    let cible = dossier_personnages(&jeu).join(&nom_fichier);
    if cible.exists() {
        let sv = dossiers.sauvegardes("personnages");
        std::fs::create_dir_all(&sv).map_err(|e| e.to_string())?;
        std::fs::copy(&cible, sv.join(format!("{nom_fichier}.{}", crate::traduction::horodatage_fichier()))).map_err(|e| format!("Sauvegarde impossible : {e}"))?;
    }
    crate::traduction::ecrire_sur(&cible, &octets).map_err(|e| e.to_string())?;
    Ok(format!("« {} » est dans le jeu : il apparaît dans la création de personnage.", nom_fichier.trim_end_matches(".chf")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_et_liste() {
        assert_eq!(nom_sur("jhon-wick.chf").as_deref(), Some("jhon-wick.chf"));
        assert_eq!(nom_sur("../../evil").as_deref(), Some("evil.chf"));
        assert_eq!(nom_sur("  ").as_deref(), None);
        let base = std::env::temp_dir().join(format!("cmp-perso-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let d = dossier_personnages(&base);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("b.chf"), [1u8; 4096]).unwrap();
        std::fs::write(d.join("a.chf"), [2u8; 4096]).unwrap();
        std::fs::write(d.join("note.txt"), b"x").unwrap();
        let l = lister(&base);
        assert_eq!(l.iter().map(|x| x.nom.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(l[0].taille, 4096);
        let _ = std::fs::remove_dir_all(&base);
    }
}
