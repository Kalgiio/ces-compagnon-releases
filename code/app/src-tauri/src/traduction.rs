//! Traduction du jeu — LECTURE SEULE (feuille de route § 1).
//!
//! - Langue active : `user.cfg` (`g_language`).
//! - Fichier installé : `data\Localization\<langue>\global.ini` (taille, date, entrées, source d'après l'en-tête).
//! - « À jour ? » : empreinte Git du fichier local (`git hash-object`) comparée à celle du fichier de la source sur
//!   GitHub, SANS le télécharger (API GitHub sans jeton : 60 requêtes/heure/IP ; une vérification toutes les 6 h).
//!
//! L'installation (plus bas) adapte le fichier pour que le jeu le lise : voir `adapter_pour_le_jeu`.

use crate::portail::client;
use serde::Serialize;
use sha1::{Digest, Sha1};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

/// Source SCEFRA (SPEED0U) — licence CC BY-NC-ND 4.0, crédit « SPEED0U ».
const SCEFRA_DEPOT: &str = "SPEED0U/Scefra";
const SCEFRA_CHEMIN: &str = "french_(france)/global.ini";
const VERIFICATION_TOUTES: Duration = Duration::from_secs(6 * 3600);

#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Distant {
    pub source: String,
    pub a_jour: Option<bool>,
    pub date: Option<String>,
    pub message: Option<String>,
    pub taille: Option<u64>,
    pub verifie_a: Option<String>,
    pub erreur: Option<String>,
}

#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EtatTraduction {
    pub langue: Option<String>,
    pub langue_audio: Option<String>,
    pub fichier: Option<String>,
    pub present: bool,
    pub taille: Option<u64>,
    /// Date de modification du fichier installé (ISO UTC).
    pub modifie: Option<String>,
    pub entrees: Option<u64>,
    /// `scefra`, `circuspes`, `inconnue`.
    pub source: Option<String>,
    pub licence: Option<String>,
    /// Fichier SCEFRA installé tel quel (sans BOM, en-tête en « # ») : le jeu ne le lit pas, à réinstaller.
    pub a_adapter: bool,
    pub distant: Option<Distant>,
}

#[derive(Default)]
pub struct Traduction {
    cache: Mutex<Option<(Instant, String, Distant)>>, // (quand, empreinte locale vérifiée, résultat)
}

fn lire_user_cfg(dossier: &Path) -> (Option<String>, Option<String>) {
    let Ok(t) = std::fs::read_to_string(dossier.join("user.cfg")) else { return (None, None) };
    let valeur = |cle: &str| {
        t.lines().find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim().eq_ignore_ascii_case(cle)).then(|| v.trim().to_string())
        })
    };
    (valeur("g_language"), valeur("g_languageAudio"))
}

/// En-tête du fichier (lignes de commentaire du début) → (source, licence).
fn reconnaitre(entete: &str) -> (String, Option<String>) {
    let bas = entete.to_lowercase();
    let licence = entete.lines().find_map(|l| l.split_once("License:").map(|(_, v)| v.trim().to_string()));
    let source = if bas.contains("speed0u") || bas.contains("scefra") {
        "scefra"
    } else if bas.contains("circuspes") {
        "circuspes"
    } else {
        "inconnue"
    };
    (source.to_string(), licence)
}

fn verifier_scefra(empreinte: &str) -> Distant {
    let mut d = Distant { source: "scefra".into(), verifie_a: Some(crate::jeu::iso_maintenant()), ..Default::default() };
    let base = format!("https://api.github.com/repos/{SCEFRA_DEPOT}");
    let lire = |url: String| -> Result<serde_json::Value, String> {
        let r = client().get(url).header("Accept", "application/vnd.github+json").send().map_err(|e| e.to_string())?;
        let statut = r.status();
        let v: serde_json::Value = r.json().map_err(|e| e.to_string())?;
        if !statut.is_success() {
            return Err(v["message"].as_str().unwrap_or("erreur GitHub").to_string());
        }
        Ok(v)
    };
    match lire(format!("{base}/contents/{SCEFRA_CHEMIN}?ref=main")) {
        Ok(v) => {
            d.taille = v["size"].as_u64();
            d.a_jour = v["sha"].as_str().map(|s| s == empreinte);
        }
        Err(e) => {
            d.erreur = Some(format!("Vérification impossible : {e}"));
            return d;
        }
    }
    if let Ok(v) = lire(format!("{base}/commits?path={SCEFRA_CHEMIN}&per_page=1")) {
        d.date = v[0]["commit"]["committer"]["date"].as_str().map(str::to_string);
        d.message = v[0]["commit"]["message"].as_str().map(|m| m.lines().next().unwrap_or("").to_string());
    }
    d
}

fn iso_fichier(t: SystemTime) -> Option<String> {
    let d = t.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    Some(crate::jeu::iso_depuis(d.as_secs() as i64, d.subsec_millis()))
}

/// État complet. `forcer` : ignore le cache de la vérification distante.
pub fn etat(dossier_jeu: Option<&Path>, t: &Traduction, forcer: bool) -> EtatTraduction {
    let Some(dossier) = dossier_jeu else { return EtatTraduction::default() };
    let (langue, langue_audio) = lire_user_cfg(dossier);
    let mut e = EtatTraduction { langue: langue.clone(), langue_audio, ..Default::default() };
    let Some(l) = langue.filter(|l| !l.eq_ignore_ascii_case("english")) else { return e };
    let chemin: PathBuf = dossier.join("data").join("Localization").join(&l).join("global.ini");
    e.fichier = Some(chemin.display().to_string());
    let Ok(meta) = std::fs::metadata(&chemin) else { return e };
    e.present = true;
    e.taille = Some(meta.len());
    e.modifie = meta.modified().ok().and_then(iso_fichier);

    // En-tête + nombre d'entrées (lignes « cle=valeur »), en flux.
    if let Ok(f) = std::fs::File::open(&chemin) {
        let mut entete = String::new();
        let mut entrees = 0u64;
        for (i, ligne) in BufReader::new(f).lines().map_while(Result::ok).enumerate() {
            let l = ligne.trim_start_matches('\u{feff}');
            if l.starts_with('#') || l.starts_with(';') {
                if i < 20 {
                    entete.push_str(l);
                    entete.push('\n');
                }
            } else if l.contains('=') {
                entrees += 1;
            }
        }
        let (source, licence) = reconnaitre(&entete);
        e.source = Some(source);
        e.licence = licence;
        e.entrees = Some(entrees);
    }

    // Vérification distante : seulement pour une traduction française (source de référence SCEFRA).
    if l.to_lowercase().starts_with("french") {
        // Le fichier installé est adapté pour le jeu (BOM, « ; ») : on compare l'original SCEFRA qu'il contient.
        let Ok(octets) = std::fs::read(&chemin) else { return e };
        let empreinte = empreinte_octets(&retrouver_original(&octets));
        e.a_adapter = e.source.as_deref() == Some("scefra") && adapter_pour_le_jeu(&octets) != octets;
        let mut cache = t.cache.lock().unwrap();
        let valide = cache.as_ref().is_some_and(|(quand, emp, d)| {
            !forcer && emp == &empreinte && d.erreur.is_none() && quand.elapsed() < VERIFICATION_TOUTES
        });
        if !valide {
            let d = verifier_scefra(&empreinte);
            *cache = Some((Instant::now(), empreinte, d));
        }
        e.distant = cache.as_ref().map(|(_, _, d)| d.clone());
    }
    e
}

#[tauri::command(async)]
pub fn traduction_etat(
    surveillance: tauri::State<'_, crate::jeu::Surveillance>,
    traduction: tauri::State<'_, Traduction>,
    forcer: Option<bool>,
) -> EtatTraduction {
    let dossier = surveillance.etat.lock().unwrap().dossier_jeu.clone().map(PathBuf::from);
    etat(dossier.as_deref(), &traduction, forcer.unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnait_scefra_et_sa_licence() {
        let (s, l) = reconnaitre("# French translation by SPEED0U / SCEFRA\n# License: CC BY-NC-ND 4.0\n");
        assert_eq!(s, "scefra");
        assert_eq!(l.as_deref(), Some("CC BY-NC-ND 4.0"));
        assert_eq!(reconnaitre("# rien\n").0, "inconnue");
    }

    #[test]
    fn empreinte_identique_a_git() {
        // `printf 'hello\n' | git hash-object --stdin` = ce013625030ba8dba906f756967f9e9ca394464a
        assert_eq!(empreinte_octets(b"hello\n"), "ce013625030ba8dba906f756967f9e9ca394464a");
    }

    #[test]
    fn adaptation_pour_le_jeu_reversible() {
        let original = b"# French translation by SPEED0U / SCEFRA\n# License: CC BY-NC-ND 4.0\ncle=Valeur # pas un commentaire\r\n# =x\r\n";
        let adapte = adapter_pour_le_jeu(original);
        assert!(adapte.starts_with(b"\xEF\xBB\xBF; French translation by SPEED0U / SCEFRA\n; License: CC BY-NC-ND 4.0\ncle="));
        assert!(adapte.ends_with(b"# =x\r\n"), "seul l'en-tête change");
        assert_eq!(retrouver_original(&adapte), original.to_vec());
        assert_eq!(adapter_pour_le_jeu(&adapte), adapte, "adapter deux fois ne change rien");
    }

    #[test]
    fn textes_multitool_remplaces_et_reversibles() {
        let original = "# French translation by SPEED0U / SCEFRA\na=1\r\nFrontend_PU_SubHeader_Desc=Merci d’utiliser l’application Multitool\r\nJournal_General_ContractManagerTutorial_Content=<EM4>Bienvenue Multitool</EM4>\\n<EM4>Multitool :</EM4>\\n<EM4>Traduction :</EM4>\\nLégende\r\nJournal_General_ContractManagerTutorial_Title=Information Multitool & Traduction\r\nz=2\r\n".as_bytes();
        let adapte = adapter_pour_le_jeu(original);
        let t = String::from_utf8(adapte.clone()).unwrap();
        assert!(t.contains("\nFrontend_PU_SubHeader_Desc=Traduction française par la team SCEFRA, installée et tenue à jour par CES Compagnon."));
        assert!(t.contains("\nJournal_General_ContractManagerTutorial_Title=CES Compagnon & Traduction\r\n"));
        let journal = t.lines().find(|l| l.starts_with("Journal_General_ContractManagerTutorial_Content=")).unwrap();
        assert!(!journal.contains("Multitool") && journal.ends_with("<EM4>Traduction :</EM4>\\nLégende"), "{journal}");
        // Lignes d'origine gardées en commentaires dans l'en-tête, avant la première traduction.
        assert!(t.starts_with("\u{feff}; French translation by SPEED0U / SCEFRA\n;ces-original:Frontend_PU_SubHeader_Desc=Merci"));
        assert_eq!(retrouver_original(&adapte), original.to_vec());
        assert_eq!(adapter_pour_le_jeu(&adapte), adapte);
    }
}

// --- Installation sur clic (décisions du CES n° 1 et n° 2 : SCEFRA ; depuis le 21/11, adapté au format du jeu sans toucher aux traductions) ---------------

const SCEFRA_BRUT: &str = "https://raw.githubusercontent.com/SPEED0U/Scefra/refs/heads/main/french_(france)/global.ini";
const LANGUE_FR: &str = "french_(france)";
const SAUVEGARDES_GARDEES: usize = 5;

/// Dossier des données de l'application (`%APPDATA%\fr.ces-sc.compagnon\`).
pub struct Dossiers {
    pub donnees: PathBuf,
}

impl Dossiers {
    pub fn sauvegardes(&self, sorte: &str) -> PathBuf {
        self.donnees.join("sauvegardes").join(sorte)
    }
}

pub fn horodatage_fichier() -> String {
    crate::jeu::iso_maintenant().replace([':', '.'], "-")
}

/// Garde les `n` plus récents fichiers commençant par `prefixe` (noms horodatés : ordre alphabétique = chronologique).
pub fn elaguer(dossier: &Path, prefixe: &str, n: usize) {
    if let Ok(rd) = std::fs::read_dir(dossier) {
        let mut noms: Vec<PathBuf> = rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file() && p.file_name().is_some_and(|f| f.to_string_lossy().starts_with(prefixe)))
            .collect();
        noms.sort();
        let trop = noms.len().saturating_sub(n);
        for p in noms.into_iter().take(trop) {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// Écrit un fichier en deux temps (fichier temporaire à côté, puis remplacement) : jamais de fichier à moitié écrit.
pub fn ecrire_sur(chemin: &Path, contenu: &[u8]) -> std::io::Result<()> {
    if let Some(d) = chemin.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = chemin.with_extension("ces-tmp");
    std::fs::write(&tmp, contenu)?;
    std::fs::rename(&tmp, chemin)
}

fn sha_attendu() -> Result<String, String> {
    let r = client()
        .get(format!("https://api.github.com/repos/{SCEFRA_DEPOT}/contents/{SCEFRA_CHEMIN}?ref=main"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| format!("GitHub injoignable : {e}"))?;
    let v: serde_json::Value = r.json().map_err(|e| e.to_string())?;
    v["sha"].as_str().map(str::to_string).ok_or_else(|| "Empreinte du fichier SCEFRA introuvable.".into())
}

const BOM: &[u8] = b"\xEF\xBB\xBF";

/// Applique `f` au premier octet de chaque ligne de commentaire de l'en-tête (avant la première ligne « clé=valeur »).
fn en_tete(contenu: &mut [u8], f: impl Fn(u8) -> u8) {
    let mut debut = 0;
    while debut < contenu.len() && matches!(contenu[debut], b'#' | b';') {
        contenu[debut] = f(contenu[debut]);
        match contenu[debut..].iter().position(|&o| o == b'\n') {
            Some(n) => debut += n + 1,
            None => break,
        }
    }
}

/// Ligne de commentaire qui garde, dans le fichier installé, une ligne d'origine remplacée (pour retrouver l'original).
const MARQUE_ORIGINAL: &[u8] = b";ces-original:";

const SOUS_TITRE_CES: &str = "Traduction française par la team SCEFRA, installée et tenue à jour par CES Compagnon. \\n\\nBon vol, pilote ! — Corps Expéditionnaire Spatial";
const JOURNAL_TITRE_CES: &str = "CES Compagnon & Traduction";
const JOURNAL_INTRO_CES: &str = "<EM4>Bienvenue sur la page d'information de la traduction française !</EM4>\\n\\n- Traduction réalisée par la Team SCEFRA (github.com/SPEED0U/Scefra, licence CC BY-NC-ND 4.0).\\n- Une faute, une suggestion ? Partagez-la sur le Discord SCEFRA (pontoon.scefra.fr pour contribuer).\\n- Veuillez noter que tous les noms des ressources, lieux et certains autres éléments sont en anglais.\\n\\n\\n<EM4>CES Compagnon :</EM4>\\nLa traduction est installée et tenue à jour depuis l'onglet Traduction de CES Compagnon, l'application du Corps Expéditionnaire Spatial (ces-sc.fr).\\n\\n_______________________________________\\n\\n";

/// Textes de SCEFRA remplacés sur le PC (décision du CES, 21/11 : message d'accueil et page du journal qui renvoyaient à
/// Multitool). Renvoie la nouvelle valeur, ou None pour garder la ligne. La page du journal garde la partie
/// « Traduction » de SCEFRA (légende des titres de missions) : seule l'introduction Multitool est remplacée.
fn texte_ces(cle: &str, valeur: &str) -> Option<String> {
    match cle {
        "Frontend_PU_SubHeader_Desc" => Some(SOUS_TITRE_CES.into()),
        "Journal_General_ContractManagerTutorial_Title" => Some(JOURNAL_TITRE_CES.into()),
        "Journal_General_ContractManagerTutorial_Content" => {
            valeur.find("<EM4>Traduction :</EM4>").map(|i| format!("{JOURNAL_INTRO_CES}{}", &valeur[i..]))
        }
        _ => None,
    }
}

/// Coupe en lignes (fin de ligne comprise).
fn lignes(contenu: &[u8]) -> Vec<&[u8]> {
    contenu.split_inclusive(|&o| o == b'\n').collect()
}

/// (ligne sans sa fin, fin de ligne)
fn sans_fin(l: &[u8]) -> (&[u8], &[u8]) {
    let n = l.len() - l.iter().rev().take_while(|&&o| o == b'\n' || o == b'\r').count();
    (&l[..n], &l[n..])
}

/// Le fichier SCEFRA tel que le jeu sait le lire (décision du CES, 21/11 : le jeu affichait toutes les clés « @… ») :
/// marque UTF-8 (BOM) au début, lignes de licence de l'en-tête en commentaires « ; » (format des fichiers du jeu) au
/// lieu de « # », et les textes Multitool remplacés par ceux du CES (`texte_ces`). Les lignes remplacées sont gardées en
/// commentaires dans l'en-tête, pour retrouver l'original. La mention de licence reste ; rien n'est republié.
pub fn adapter_pour_le_jeu(contenu: &[u8]) -> Vec<u8> {
    let mut v = retrouver_original(contenu);
    en_tete(&mut v, |_| b';');
    let mut gardees: Vec<u8> = Vec::new();
    let mut corps: Vec<u8> = Vec::with_capacity(v.len() + 2048);
    let mut fin_en_tete = None;
    for l in lignes(&v) {
        let (texte, fin) = sans_fin(l);
        if fin_en_tete.is_none() && !texte.starts_with(b";") {
            fin_en_tete = Some(corps.len());
        }
        let remplacement = std::str::from_utf8(texte).ok().and_then(|t| {
            let (k, val) = t.split_once('=')?;
            texte_ces(k, val).map(|n| format!("{k}={n}"))
        });
        match remplacement {
            Some(n) => {
                gardees.extend_from_slice(MARQUE_ORIGINAL);
                gardees.extend_from_slice(texte);
                gardees.extend_from_slice(b"\r\n");
                corps.extend_from_slice(n.as_bytes());
                corps.extend_from_slice(fin);
            }
            None => corps.extend_from_slice(l),
        }
    }
    let i = fin_en_tete.unwrap_or(corps.len());
    [BOM, &corps[..i], &gardees, &corps[i..]].concat()
}

/// Inverse d'`adapter_pour_le_jeu` : retrouve le fichier publié par SCEFRA, pour comparer son empreinte.
fn retrouver_original(contenu: &[u8]) -> Vec<u8> {
    let v = contenu.strip_prefix(BOM).unwrap_or(contenu);
    let mut originaux: Vec<(&[u8], &[u8])> = Vec::new(); // (clé, ligne d'origine sans fin)
    for l in lignes(v) {
        if let Some(o) = sans_fin(l).0.strip_prefix(MARQUE_ORIGINAL) {
            if let Some(i) = o.iter().position(|&c| c == b'=') {
                originaux.push((&o[..i], o));
            }
        }
    }
    let mut sortie = Vec::with_capacity(v.len());
    for l in lignes(v) {
        let (texte, fin) = sans_fin(l);
        if texte.starts_with(MARQUE_ORIGINAL) {
            continue;
        }
        let cle = texte.iter().position(|&c| c == b'=').map(|i| &texte[..i]);
        match originaux.iter().find(|(k, _)| Some(*k) == cle) {
            Some((_, o)) => {
                sortie.extend_from_slice(o);
                sortie.extend_from_slice(fin);
            }
            None => sortie.extend_from_slice(l),
        }
    }
    en_tete(&mut sortie, |_| b'#');
    sortie
}

fn empreinte_octets(contenu: &[u8]) -> String {
    let mut h = Sha1::new();
    h.update(format!("blob {}\0", contenu.len()).as_bytes());
    h.update(contenu);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Met `g_language = french_(france)` dans `user.cfg` si besoin (sauvegarde avant). Renvoie vrai si modifié.
fn regler_user_cfg(dossier_jeu: &Path, sauvegardes: &Path) -> Result<bool, String> {
    let chemin = dossier_jeu.join("user.cfg");
    let actuel = std::fs::read_to_string(&chemin).unwrap_or_default();
    let est_langue = |l: &str| l.split_once('=').is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case("g_language"));
    if actuel.lines().any(|l| est_langue(l) && l.split_once('=').is_some_and(|(_, v)| v.trim() == LANGUE_FR)) {
        return Ok(false);
    }
    if chemin.exists() {
        std::fs::create_dir_all(sauvegardes).map_err(|e| e.to_string())?;
        std::fs::copy(&chemin, sauvegardes.join(format!("user.cfg.{}", horodatage_fichier()))).map_err(|e| e.to_string())?;
    }
    let mut lignes: Vec<String> = actuel.lines().filter(|l| !est_langue(l)).map(str::to_string).collect();
    lignes.push(format!("g_language = {LANGUE_FR}"));
    ecrire_sur(&chemin, (lignes.join("\r\n") + "\r\n").as_bytes()).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Télécharge le `global.ini` de SCEFRA, vérifie son intégrité, sauvegarde l'ancien, l'installe.
pub fn installer(dossier_jeu: &Path, dossiers: &Dossiers) -> Result<String, String> {
    let attendu = sha_attendu()?;
    let r = client()
        .get(SCEFRA_BRUT)
        .timeout(Duration::from_secs(180))
        .send()
        .map_err(|e| format!("Téléchargement impossible : {e}"))?;
    if !r.status().is_success() {
        return Err(format!("Téléchargement refusé ({}).", r.status()));
    }
    let contenu = r.bytes().map_err(|e| format!("Téléchargement interrompu : {e}"))?;
    if empreinte_octets(&contenu) != attendu {
        return Err("Le fichier reçu ne correspond pas à celui publié par SCEFRA : installation annulée.".into());
    }

    let sauvegardes = dossiers.sauvegardes("traduction");
    let cible = dossier_jeu.join("data").join("Localization").join(LANGUE_FR).join("global.ini");
    if cible.exists() {
        std::fs::create_dir_all(&sauvegardes).map_err(|e| e.to_string())?;
        std::fs::copy(&cible, sauvegardes.join(format!("global.ini.{}", horodatage_fichier())))
            .map_err(|e| format!("Sauvegarde impossible, rien n'a été modifié : {e}"))?;
    }
    ecrire_sur(&cible, &adapter_pour_le_jeu(&contenu)).map_err(|e| format!("Écriture impossible : {e}"))?;
    let cfg = regler_user_cfg(dossier_jeu, &sauvegardes)?;
    elaguer(&sauvegardes, "global.ini.", SAUVEGARDES_GARDEES);
    elaguer(&sauvegardes, "user.cfg.", SAUVEGARDES_GARDEES);
    Ok(if cfg { "Traduction installée, et le jeu est réglé en français (user.cfg).".into() } else { "Traduction installée.".into() })
}

/// Remet la dernière sauvegarde de `global.ini` (l'actuel est lui-même sauvegardé avant).
pub fn restaurer(dossier_jeu: &Path, dossiers: &Dossiers) -> Result<String, String> {
    let sauvegardes = dossiers.sauvegardes("traduction");
    let mut liste: Vec<PathBuf> = std::fs::read_dir(&sauvegardes)
        .map_err(|_| "Aucune sauvegarde de traduction.".to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("global.ini.")))
        .collect();
    liste.sort();
    let derniere = liste.pop().ok_or("Aucune sauvegarde de traduction.")?;
    let contenu = std::fs::read(&derniere).map_err(|e| e.to_string())?;
    let cible = dossier_jeu.join("data").join("Localization").join(LANGUE_FR).join("global.ini");
    let actuel = std::fs::read(&cible).ok();
    ecrire_sur(&cible, &contenu).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&derniere); // restaurée : on ne la repropose pas
    if let Some(a) = actuel {
        // la version remplacée devient la sauvegarde la plus récente (on peut revenir en avant)
        let _ = std::fs::write(sauvegardes.join(format!("global.ini.{}", horodatage_fichier())), a);
    }
    Ok("Traduction précédente restaurée.".into())
}

pub fn nb_sauvegardes(dossiers: &Dossiers) -> usize {
    std::fs::read_dir(dossiers.sauvegardes("traduction"))
        .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().starts_with("global.ini.")).count())
        .unwrap_or(0)
}

pub fn verifier_jeu_ferme(surveillance: &crate::jeu::Surveillance, quoi: &str) -> Result<PathBuf, String> {
    let e = surveillance.etat.lock().unwrap();
    if e.jeu_lance {
        return Err(format!("Ferme Star Citizen avant de {quoi}."));
    }
    e.dossier_jeu.clone().map(PathBuf::from).ok_or_else(|| "Dossier du jeu introuvable.".into())
}

#[tauri::command(async)]
pub fn traduction_installer(
    surveillance: tauri::State<'_, crate::jeu::Surveillance>,
    dossiers: tauri::State<'_, Dossiers>,
    traduction: tauri::State<'_, Traduction>,
) -> Result<String, String> {
    let dossier = verifier_jeu_ferme(&surveillance, "changer la traduction")?;
    let r = installer(&dossier, &dossiers);
    *traduction.cache.lock().unwrap() = None; // revérifier au prochain affichage
    r
}

#[tauri::command(async)]
pub fn traduction_restaurer(
    surveillance: tauri::State<'_, crate::jeu::Surveillance>,
    dossiers: tauri::State<'_, Dossiers>,
    traduction: tauri::State<'_, Traduction>,
) -> Result<String, String> {
    let dossier = verifier_jeu_ferme(&surveillance, "changer la traduction")?;
    let r = restaurer(&dossier, &dossiers);
    *traduction.cache.lock().unwrap() = None;
    r
}

#[tauri::command]
pub fn traduction_sauvegardes(dossiers: tauri::State<'_, Dossiers>) -> usize {
    nb_sauvegardes(&dossiers)
}

#[cfg(test)]
mod tests_installation {
    use super::*;

    fn dossier(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cmp-{nom}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn user_cfg_regle_et_sauvegarde() {
        let base = dossier("cfg");
        std::fs::write(base.join("user.cfg"), "r_displayInfo = 0\ng_language = english\n").unwrap();
        let sv = base.join("sv");
        assert!(regler_user_cfg(&base, &sv).unwrap());
        let t = std::fs::read_to_string(base.join("user.cfg")).unwrap();
        assert!(t.contains("r_displayInfo = 0") && t.contains("g_language = french_(france)") && !t.contains("english"));
        assert_eq!(std::fs::read_dir(&sv).unwrap().count(), 1);
        assert!(!regler_user_cfg(&base, &sv).unwrap()); // déjà réglé : rien ne change
    }

    #[test]
    fn restaurer_puis_revenir() {
        let jeu = dossier("jeu");
        let d = Dossiers { donnees: dossier("donnees") };
        let cible = jeu.join("data").join("Localization").join(LANGUE_FR).join("global.ini");
        ecrire_sur(&cible, b"ancienne").unwrap();
        std::fs::create_dir_all(d.sauvegardes("traduction")).unwrap();
        std::fs::copy(&cible, d.sauvegardes("traduction").join("global.ini.2026-01-01T00-00-00-000Z")).unwrap();
        ecrire_sur(&cible, b"nouvelle").unwrap();
        restaurer(&jeu, &d).unwrap();
        assert_eq!(std::fs::read(&cible).unwrap(), b"ancienne");
        assert_eq!(nb_sauvegardes(&d), 1); // la « nouvelle » est gardée pour revenir en avant
        restaurer(&jeu, &d).unwrap();
        assert_eq!(std::fs::read(&cible).unwrap(), b"nouvelle");
    }
}

#[cfg(test)]
mod test_reel {
    use super::*;

    /// Téléchargement réel depuis SCEFRA, installé dans une COPIE de dossier du jeu (jamais le vrai).
    /// cargo test --lib -- --ignored installer_sur_copie
    #[test]
    #[ignore]
    fn installer_sur_copie() {
        let base = std::env::temp_dir().join(format!("cmp-reel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let jeu = base.join("LIVE");
        let cible = jeu.join("data").join("Localization").join(LANGUE_FR).join("global.ini");
        ecrire_sur(&cible, b"# French translation by SPEED0U / SCEFRA\nancienne=1\n").unwrap();
        std::fs::write(jeu.join("user.cfg"), "g_language = english\n").unwrap();
        let d = Dossiers { donnees: base.join("donnees") };
        let msg = installer(&jeu, &d).unwrap();
        println!("{msg}");
        let taille = std::fs::metadata(&cible).unwrap().len();
        assert!(taille > 10_000_000, "taille {taille}");
        assert_eq!(nb_sauvegardes(&d), 1);
        assert!(std::fs::read_to_string(jeu.join("user.cfg")).unwrap().contains("g_language = french_(france)"));
        let e = etat(Some(&jeu), &Traduction::default(), true);
        assert!(std::fs::read(&cible).unwrap().starts_with(b"\xEF\xBB\xBF; French translation"), "installé au format du jeu");
        assert!(!e.a_adapter);
        let installe = std::fs::read_to_string(&cible).unwrap();
        for cle in ["Frontend_PU_SubHeader_Desc=", "Journal_General_ContractManagerTutorial_Title=", "Journal_General_ContractManagerTutorial_Content="] {
            let l = installe.lines().find(|l| l.starts_with(cle)).unwrap();
            assert!(!l.contains("Multitool") && l.contains("CES"), "texte du CES attendu : {}", &l[..l.len().min(160)]);
        }
        assert_eq!(e.distant.and_then(|d| d.a_jour), Some(true), "après installation, le fichier doit être à jour");
        restaurer(&jeu, &d).unwrap();
        assert!(std::fs::metadata(&cible).unwrap().len() < 100);
        let _ = std::fs::remove_dir_all(&base);
    }
}
