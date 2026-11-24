//! Partage des profils de touches entre membres (feuille de route § 2, décision du CES n° 4 : l'auteur choisit
//! « tous les membres CES » ou « membres désignés »). Contrat : `API.md` § 5 ter.
//!
//! - Publier : le profil actuel (ou une sauvegarde) part au portail — c'est le fichier natif du jeu, qui ne contient
//!   que des noms d'actions et de touches (aucune donnée personnelle).
//! - Récupérer un profil partagé : il devient une SAUVEGARDE locale ; rien n'est appliqué au jeu sans « Restaurer ».

use crate::portail::{self, Erreur};
use crate::presence::{jeton_actuel, Portail};
use crate::touches::{self, analyser};
use crate::traduction::{elaguer, horodatage_fichier, Dossiers};
use reqwest::Method;
use serde_json::{json, Value};
use std::path::PathBuf;

const XML_MAX: usize = 256 * 1024;

fn message(e: Erreur) -> String {
    if e.jeton_refuse() {
        "Ton appareil n'est plus relié au portail : relie-le à nouveau (onglet Compte).".into()
    } else {
        e.to_string() // ex. 400 invalid_xml : message lisible du portail, affiché tel quel
    }
}

/// Appareils du profil → valeurs du contrat (`keyboard`, `mouse`, `gamepad`, `joystick`).
fn appareils(xml: &str) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    if let Ok(p) = analyser(xml) {
        for t in &p.touches {
            let a = match t.appareil.as_str() {
                "clavier" => "keyboard",
                "souris" => "mouse",
                "manette" => "gamepad",
                s if s.starts_with("joystick") => "joystick",
                _ => continue,
            };
            if !v.contains(&a) {
                v.push(a);
            }
        }
    }
    v
}

#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
pub fn touches_publier(
    portail_etat: tauri::State<'_, Portail>,
    surveillance: tauri::State<'_, crate::jeu::Surveillance>,
    dossiers: tauri::State<'_, Dossiers>,
    nom: String,
    description: Option<String>,
    visibilite: String,
    destinataires: Vec<String>,
    sauvegarde: Option<String>,
) -> Result<String, String> {
    let jeton = jeton_actuel(&portail_etat)?;
    let (dossier_jeu, version) = {
        let e = surveillance.etat.lock().unwrap();
        (e.dossier_jeu.clone().map(PathBuf::from), e.version.clone())
    };
    let source = match sauvegarde {
        Some(n) => touches::fichier_sauvegarde(&dossiers, &n)?,
        None => touches::chemin_profil(&dossier_jeu.ok_or("Dossier du jeu introuvable.")?),
    };
    let xml = std::fs::read_to_string(&source).map_err(|_| "Profil de touches introuvable.".to_string())?;
    if xml.len() > XML_MAX {
        return Err("Profil trop gros pour être partagé (256 Ko au plus).".into());
    }
    let p = analyser(&xml)?;
    if p.touches.is_empty() {
        return Err("Ce profil ne contient aucune touche modifiée : rien à partager.".into());
    }
    let nom: String = nom.trim().chars().take(80).collect();
    if nom.is_empty() {
        return Err("Donne un nom à ton profil.".into());
    }
    if visibilite != "ces" && visibilite != "members" {
        return Err("Visibilité inconnue.".into());
    }
    if visibilite == "members" && destinataires.is_empty() {
        return Err("Choisis au moins un membre, ou partage avec tout le CES.".into());
    }
    let mut corps = json!({
        "name": nom,
        "devices": appareils(&xml),
        "xml": xml,
        "visibility": visibilite,
    });
    if let Some(d) = description.map(|d| d.trim().chars().take(500).collect::<String>()).filter(|d| !d.is_empty()) {
        corps["description"] = json!(d);
    }
    if let Some(v) = version {
        corps["gameVersion"] = json!(v);
    }
    if visibilite == "members" {
        corps["sharedWith"] = json!(destinataires);
    }
    portail::appel(&jeton, Method::POST, "bindings", Some(&corps)).map_err(message)?;
    Ok(if visibilite == "ces" {
        format!("« {nom} » est partagé avec tous les membres du CES.")
    } else {
        format!("« {nom} » est partagé avec {} membre(s).", destinataires.len())
    })
}

#[tauri::command(async)]
pub fn touches_partages(portail_etat: tauri::State<'_, Portail>, miens: Option<bool>) -> Result<Value, String> {
    let jeton = jeton_actuel(&portail_etat)?;
    let chemin = if miens.unwrap_or(false) { "bindings?mine=1" } else { "bindings" };
    portail::appel(&jeton, Method::GET, chemin, None).map(|v| v["bindings"].clone()).map_err(message)
}

#[tauri::command(async)]
pub fn touches_membres(portail_etat: tauri::State<'_, Portail>) -> Result<Value, String> {
    let jeton = jeton_actuel(&portail_etat)?;
    portail::appel(&jeton, Method::GET, "members", None).map(|v| v["members"].clone()).map_err(message)
}

fn id_sur(id: &str) -> Result<&str, String> {
    if id.is_empty() || id.len() > 64 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err("Profil inconnu.".into());
    }
    Ok(id)
}

/// Récupère un profil partagé : il devient une sauvegarde locale (jamais appliqué directement).
#[tauri::command(async)]
pub fn touches_recuperer(portail_etat: tauri::State<'_, Portail>, dossiers: tauri::State<'_, Dossiers>, id: String) -> Result<String, String> {
    let jeton = jeton_actuel(&portail_etat)?;
    let v = portail::appel(&jeton, Method::GET, &format!("bindings/{}", id_sur(&id)?), None).map_err(message)?;
    let xml = v["xml"].as_str().ok_or("Profil vide.")?;
    if xml.len() > XML_MAX {
        return Err("Profil trop gros.".into());
    }
    let p = analyser(xml)?; // jamais un fichier invalide dans nos sauvegardes
    let nom = v["name"].as_str().unwrap_or("profil partagé");
    let auteur = v["author"]["username"].as_str().unwrap_or("membre");
    let libelle: String = format!("{nom} par {auteur}")
        .chars()
        .filter(|c| c.is_alphanumeric() || " -_".contains(*c))
        .take(40)
        .collect::<String>()
        .trim()
        .replace(' ', "-");
    let dossier = dossiers.sauvegardes("touches");
    std::fs::create_dir_all(&dossier).map_err(|e| e.to_string())?;
    std::fs::write(dossier.join(format!("actionmaps.{}.{}.xml", horodatage_fichier(), libelle)), xml).map_err(|e| e.to_string())?;
    elaguer(&dossier, "actionmaps.", 30);
    Ok(format!("« {nom} » ({} touches) ajouté à tes sauvegardes : restaure-le quand tu veux, jeu fermé.", p.touches.len()))
}

#[tauri::command(async)]
pub fn touches_retirer_partage(portail_etat: tauri::State<'_, Portail>, id: String) -> Result<String, String> {
    let jeton = jeton_actuel(&portail_etat)?;
    portail::appel(&jeton, Method::DELETE, &format!("bindings/{}", id_sur(&id)?), None).map_err(message)?;
    Ok("Profil retiré du partage.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appareils_du_contrat() {
        let xml = r#"<ActionMaps><ActionProfiles profileName="default"><actionmap name="a">
          <action name="x"><rebind input="kb1_mouse5"/></action>
          <action name="y"><rebind input="js1_button2"/></action>
          <action name="z"><rebind input="kb1_f"/></action></actionmap></ActionProfiles></ActionMaps>"#;
        assert_eq!(appareils(xml), vec!["mouse", "joystick", "keyboard"]);
        assert!(id_sur("../x").is_err());
        assert!(id_sur("3f2a-9c").is_ok());
    }
}
