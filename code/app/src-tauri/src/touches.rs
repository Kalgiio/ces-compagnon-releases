//! Touches du jeu (feuille de route § 2) : `user\client\0\Profiles\default\actionmaps.xml`.
//!
//! Ce fichier ne contient que les touches MODIFIÉES par le joueur (décision n° 3 : pas de lecture de `Data.p4k`).
//! - Lecture : liste lisible par catégorie et par appareil.
//! - Sauvegardes datées dans `%APPDATA%\fr.ces-sc.compagnon\sauvegardes\touches\`.
//! - Restauration : sur clic, JEU FERMÉ (le jeu réécrit ce fichier en quittant), avec sauvegarde de l'actuel.
//! - Export / import d'un profil (format natif du jeu). Un import n'est jamais appliqué directement : il devient une
//!   sauvegarde, que le membre restaure s'il le veut.

use crate::traduction::{ecrire_sur, elaguer, horodatage_fichier, verifier_jeu_ferme, Dossiers};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri_plugin_dialog::DialogExt;

const SAUVEGARDES_GARDEES: usize = 30;
const TAILLE_MAX: u64 = 2 * 1024 * 1024;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Touche {
    pub categorie: String,
    pub action: String,
    pub entree: String,
    /// `clavier`, `souris`, `manette`, `joystick 1`…
    pub appareil: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Profil {
    pub fichier: Option<String>,
    pub present: bool,
    pub nom: Option<String>,
    pub appareils: Vec<String>,
    pub touches: Vec<Touche>,
    pub erreur: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sauvegarde {
    pub fichier: String,
    pub date: String,
    pub libelle: Option<String>,
    pub touches: usize,
}

pub(crate) fn chemin_profil(dossier_jeu: &Path) -> PathBuf {
    dossier_jeu.join("user").join("client").join("0").join("Profiles").join("default").join("actionmaps.xml")
}

fn appareil(entree: &str) -> String {
    let (prefixe, reste) = entree.split_once('_').unwrap_or((entree, ""));
    if prefixe.starts_with("kb") {
        if reste.starts_with("mouse") { "souris".into() } else { "clavier".into() }
    } else if prefixe.starts_with("mo") {
        "souris".into()
    } else if prefixe.starts_with("gp") {
        "manette".into()
    } else if let Some(n) = prefixe.strip_prefix("js") {
        format!("joystick {n}")
    } else {
        prefixe.to_string()
    }
}

fn attribut(e: &quick_xml::events::BytesStart, nom: &str) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.as_ref() == nom).map(|a| a.value.to_string())
}

/// Analyse un `actionmaps.xml` (ou un profil exporté : même format).
pub fn analyser(xml: &str) -> Result<Profil, String> {
    let mut r = Reader::from_str(xml);
    r.config_mut().trim_text(true);
    let mut p = Profil::default();
    let (mut categorie, mut action) = (String::new(), String::new());
    let mut racine_vue = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                "ActionMaps" => racine_vue = true,
                "ActionProfiles" => p.nom = attribut(&e, "profileName"),
                "options" => {
                    if let (Some(t), Some(produit)) = (attribut(&e, "type"), attribut(&e, "Product")) {
                        let produit = produit.split('{').next().unwrap_or("").trim().to_string();
                        p.appareils.push(format!("{t} : {produit}"));
                    }
                }
                "actionmap" => categorie = attribut(&e, "name").unwrap_or_default(),
                "action" => action = attribut(&e, "name").unwrap_or_default(),
                "rebind" => {
                    if let Some(entree) = attribut(&e, "input") {
                        let entree = entree.trim().to_string();
                        // « kb1_ » seul = touche retirée (aucune touche)
                        let vide = entree.ends_with('_') || entree.is_empty();
                        p.touches.push(Touche {
                            categorie: categorie.clone(),
                            action: action.clone(),
                            appareil: appareil(&entree),
                            entree: if vide { "(aucune)".into() } else { entree },
                        });
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Fichier de touches illisible : {e}")),
            _ => {}
        }
    }
    if !racine_vue {
        return Err("Ce fichier n'est pas un profil de touches de Star Citizen (balise ActionMaps absente).".into());
    }
    Ok(p)
}

pub fn lire(dossier_jeu: &Path) -> Profil {
    let chemin = chemin_profil(dossier_jeu);
    let mut p = match std::fs::read_to_string(&chemin) {
        Ok(t) => analyser(&t).unwrap_or_else(|e| Profil { erreur: Some(e), ..Default::default() }),
        Err(_) => Profil::default(),
    };
    p.present = chemin.exists();
    p.fichier = Some(chemin.display().to_string());
    p
}

fn dossier_sauvegardes(d: &Dossiers) -> PathBuf {
    d.sauvegardes("touches")
}

/// Nom de fichier sûr pour un libellé choisi par le membre.
fn libelle_sur(libelle: &str) -> String {
    libelle.chars().filter(|c| c.is_alphanumeric() || " -_".contains(*c)).take(40).collect::<String>().trim().replace(' ', "-")
}

pub fn sauvegarder(dossier_jeu: &Path, d: &Dossiers, libelle: Option<&str>) -> Result<String, String> {
    let source = chemin_profil(dossier_jeu);
    let contenu = std::fs::read(&source).map_err(|_| "Aucun fichier de touches à sauvegarder.".to_string())?;
    let dossier = dossier_sauvegardes(d);
    std::fs::create_dir_all(&dossier).map_err(|e| e.to_string())?;
    let suffixe = libelle.map(libelle_sur).filter(|s| !s.is_empty()).map(|s| format!(".{s}")).unwrap_or_default();
    let nom = format!("actionmaps.{}{}.xml", horodatage_fichier(), suffixe);
    std::fs::write(dossier.join(&nom), contenu).map_err(|e| e.to_string())?;
    elaguer(&dossier, "actionmaps.", SAUVEGARDES_GARDEES);
    Ok(nom)
}

pub fn sauvegardes(d: &Dossiers) -> Vec<Sauvegarde> {
    let Ok(rd) = std::fs::read_dir(dossier_sauvegardes(d)) else { return Vec::new() };
    let mut v: Vec<Sauvegarde> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let nom = e.file_name().to_string_lossy().into_owned();
            let milieu = nom.strip_prefix("actionmaps.")?.strip_suffix(".xml")?.to_string();
            // actionmaps.<2026-11-15T01-04-00-000Z>[.<libellé>].xml
            let (date, libelle) = match milieu.split_once('.') {
                Some((d, l)) => (d.to_string(), Some(l.replace('-', " "))),
                None => (milieu.clone(), None),
            };
            let touches = std::fs::read_to_string(e.path()).ok().and_then(|t| analyser(&t).ok()).map(|p| p.touches.len()).unwrap_or(0);
            Some(Sauvegarde { fichier: nom, date, libelle, touches })
        })
        .collect();
    v.sort_by(|a, b| b.fichier.cmp(&a.fichier));
    v
}

pub(crate) fn fichier_sauvegarde(d: &Dossiers, nom: &str) -> Result<PathBuf, String> {
    // Seulement un nom de fichier de NOTRE dossier (jamais un chemin venu de l'interface).
    if nom.contains(['/', '\\']) || nom.contains("..") || !nom.starts_with("actionmaps.") || !nom.ends_with(".xml") {
        return Err("Sauvegarde inconnue.".into());
    }
    let p = dossier_sauvegardes(d).join(nom);
    p.is_file().then_some(p).ok_or_else(|| "Sauvegarde introuvable.".into())
}

pub fn restaurer(dossier_jeu: &Path, d: &Dossiers, nom: &str) -> Result<String, String> {
    let source = fichier_sauvegarde(d, nom)?;
    let contenu = std::fs::read_to_string(&source).map_err(|e| e.to_string())?;
    analyser(&contenu)?; // jamais un fichier invalide dans le dossier du jeu
    if chemin_profil(dossier_jeu).exists() {
        sauvegarder(dossier_jeu, d, Some("avant restauration"))?;
    }
    ecrire_sur(&chemin_profil(dossier_jeu), contenu.as_bytes()).map_err(|e| format!("Écriture impossible : {e}"))?;
    Ok("Touches restaurées. Elles s'appliqueront au prochain lancement du jeu.".into())
}

// --- Commandes ------------------------------------------------------------------------------------------

fn dossier_jeu(s: &crate::jeu::Surveillance) -> Result<PathBuf, String> {
    s.etat.lock().unwrap().dossier_jeu.clone().map(PathBuf::from).ok_or_else(|| "Dossier du jeu introuvable.".into())
}

#[tauri::command]
pub fn touches_lire(surveillance: tauri::State<'_, crate::jeu::Surveillance>) -> Result<Profil, String> {
    Ok(lire(&dossier_jeu(&surveillance)?))
}

#[tauri::command]
pub fn touches_sauvegardes(dossiers: tauri::State<'_, Dossiers>) -> Vec<Sauvegarde> {
    sauvegardes(&dossiers)
}

#[tauri::command]
pub fn touches_sauvegarder(
    surveillance: tauri::State<'_, crate::jeu::Surveillance>,
    dossiers: tauri::State<'_, Dossiers>,
    libelle: Option<String>,
) -> Result<String, String> {
    sauvegarder(&dossier_jeu(&surveillance)?, &dossiers, libelle.as_deref())
}

#[tauri::command]
pub fn touches_restaurer(
    surveillance: tauri::State<'_, crate::jeu::Surveillance>,
    dossiers: tauri::State<'_, Dossiers>,
    fichier: String,
) -> Result<String, String> {
    let dossier = verifier_jeu_ferme(&surveillance, "restaurer des touches")?;
    restaurer(&dossier, &dossiers, &fichier)
}

/// Exporte une sauvegarde (ou le profil actuel si `fichier` est absent) là où le membre le choisit.
#[tauri::command(async)]
pub fn touches_exporter(
    app: tauri::AppHandle,
    surveillance: tauri::State<'_, crate::jeu::Surveillance>,
    dossiers: tauri::State<'_, Dossiers>,
    fichier: Option<String>,
) -> Result<Option<String>, String> {
    let source = match fichier {
        Some(n) => fichier_sauvegarde(&dossiers, &n)?,
        None => chemin_profil(&dossier_jeu(&surveillance)?),
    };
    let contenu = std::fs::read(&source).map_err(|_| "Rien à exporter.".to_string())?;
    let Some(cible) = app
        .dialog()
        .file()
        .set_title("Exporter le profil de touches")
        .set_file_name("touches-star-citizen.xml")
        .add_filter("Profil de touches Star Citizen", &["xml"])
        .blocking_save_file()
    else {
        return Ok(None); // annulé
    };
    let chemin = cible.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&chemin, contenu).map_err(|e| e.to_string())?;
    Ok(Some(chemin.display().to_string()))
}

/// Importe un profil choisi par le membre : il devient une SAUVEGARDE (rien n'est appliqué au jeu).
#[tauri::command(async)]
pub fn touches_importer(app: tauri::AppHandle, dossiers: tauri::State<'_, Dossiers>) -> Result<Option<String>, String> {
    let Some(choix) = app
        .dialog()
        .file()
        .set_title("Importer un profil de touches")
        .add_filter("Profil de touches Star Citizen", &["xml"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let chemin = choix.into_path().map_err(|e| e.to_string())?;
    if std::fs::metadata(&chemin).map(|m| m.len()).unwrap_or(u64::MAX) > TAILLE_MAX {
        return Err("Fichier trop gros pour un profil de touches.".into());
    }
    let contenu = std::fs::read_to_string(&chemin).map_err(|_| "Fichier illisible (texte attendu).".to_string())?;
    let p = analyser(&contenu)?;
    let dossier = dossier_sauvegardes(&dossiers);
    std::fs::create_dir_all(&dossier).map_err(|e| e.to_string())?;
    let nom = format!("actionmaps.{}.importe.xml", horodatage_fichier());
    std::fs::write(dossier.join(&nom), contenu).map_err(|e| e.to_string())?;
    elaguer(&dossier, "actionmaps.", SAUVEGARDES_GARDEES);
    Ok(Some(format!("Profil importé ({} touches) : il apparaît dans tes sauvegardes, à restaurer quand tu veux.", p.touches.len())))
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<ActionMaps>
 <ActionProfiles version="1" optionsVersion="2" rebindVersion="2" profileName="default">
  <options type="keyboard" instance="1" Product="Clavier  {6F1D2B61-D5A0-11CF-BFC7-444553540000}"/>
  <options type="joystick" instance="1"/>
  <modifiers />
  <actionmap name="player_input_optical_tracking">
   <action name="foip_pushtotalk"><rebind input="kb1_mouse5"/></action>
   <action name="foip_pushtotalk_proximity"><rebind input="kb1_mouse4"/></action>
  </actionmap>
  <actionmap name="spaceship_movement">
   <action name="v_strafe_up"><rebind input="js2_button3"/></action>
   <action name="v_brake"><rebind input="kb1_ "/></action>
  </actionmap>
 </ActionProfiles>
</ActionMaps>"#;

    #[test]
    fn lit_le_profil_reel() {
        let p = analyser(XML).unwrap();
        assert_eq!(p.nom.as_deref(), Some("default"));
        assert_eq!(p.appareils, vec!["keyboard : Clavier".to_string()]);
        assert_eq!(p.touches.len(), 4);
        assert_eq!(p.touches[0].appareil, "souris");
        assert_eq!(p.touches[2].appareil, "joystick 2");
        assert_eq!(p.touches[3].entree, "(aucune)");
        assert!(analyser("<html></html>").is_err());
    }

    #[test]
    fn sauvegarder_restaurer() {
        let base = std::env::temp_dir().join(format!("cmp-touches-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let jeu = base.join("LIVE");
        ecrire_sur(&chemin_profil(&jeu), XML.as_bytes()).unwrap();
        let d = Dossiers { donnees: base.join("donnees") };
        let nom = sauvegarder(&jeu, &d, Some("Mon profil HOTAS!")).unwrap();
        assert!(nom.ends_with(".Mon-profil-HOTAS.xml"), "{nom}");
        ecrire_sur(&chemin_profil(&jeu), b"<ActionMaps></ActionMaps>").unwrap();
        restaurer(&jeu, &d, &nom).unwrap();
        assert_eq!(std::fs::read_to_string(chemin_profil(&jeu)).unwrap(), XML);
        let liste = sauvegardes(&d);
        assert_eq!(liste.len(), 2); // la sauvegarde + « avant restauration »
        assert!(restaurer(&jeu, &d, "../../Windows/x.xml").is_err());
    }
}
