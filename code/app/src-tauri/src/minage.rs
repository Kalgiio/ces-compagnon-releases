//! Workflow du minage (feuille de route § 4, décision du CES n° 6 : partage sur adhésion, réglage `mining`).
//!
//! - Saisie MANUELLE : `Game.log` ne donne ni quantités ni raffinerie.
//! - Référentiel UEX (cache disque) : minerais bruts → version raffinée et meilleur prix de vente récent,
//!   raffineries et leurs bonus de rendement par minerai, méthodes de raffinage.
//! - Sessions gardées sur le PC (SQLite `minage.db`) ; une session TERMINÉE part au portail (`mining.session`)
//!   seulement si le réglage `mining` est activé et qu'elle est postérieure à l'appairage.

use crate::commerce::charger;
use crate::presence::{envoyer_evenement, Portail};
use crate::traduction::Dossiers;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri_plugin_dialog::DialogExt;

const TTL_REF: u64 = 12 * 3600;
const TTL_PRIX: u64 = 30 * 60;
const AGE_PRIX_MAX_S: f64 = 7.0 * 86400.0;

pub struct Minage {
    conn: Mutex<Connection>,
}

impl Minage {
    pub fn ouvrir(dossiers: &Dossiers) -> rusqlite::Result<Minage> {
        let _ = std::fs::create_dir_all(&dossiers.donnees);
        let conn = Connection::open(dossiers.donnees.join("minage.db"))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS sessions (
               id TEXT PRIMARY KEY,
               debut TEXT NOT NULL,
               fin TEXT,
               vaisseau TEXT, lieu TEXT, raffinerie TEXT, methode TEXT,
               minerais TEXT NOT NULL DEFAULT '[]',
               gains REAL, notes TEXT,
               envoye INTEGER NOT NULL DEFAULT 0
             );",
        )?;
        Ok(Minage { conn: Mutex::new(conn) })
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Minerai {
    nom: String,
    scu: f64,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    id: String,
    debut: String,
    fin: Option<String>,
    vaisseau: Option<String>,
    lieu: Option<String>,
    raffinerie: Option<String>,
    methode: Option<String>,
    #[serde(default)]
    minerais: Vec<Minerai>,
    gains: Option<f64>,
    notes: Option<String>,
    #[serde(default)]
    envoye: bool,
}

fn nombre(v: &Value) -> f64 {
    v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0.0)
}

// --- Référentiel ------------------------------------------------------------------------------------------

#[tauri::command(async)]
pub fn minage_referentiel(dossiers: tauri::State<'_, Dossiers>, forcer: Option<bool>) -> Result<Value, String> {
    let forcer = forcer.unwrap_or(false);
    let (com, _) = charger(&dossiers, "commodities", TTL_REF, false)?;
    let (prix, recu) = charger(&dossiers, "commodities_prices_all", TTL_PRIX, forcer)?;
    let (rend, _) = charger(&dossiers, "refineries_yields", TTL_REF, forcer)?;
    let (meth, _) = charger(&dossiers, "refineries_methods", TTL_REF, false)?;

    let noms: HashMap<i64, String> = com
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| Some((c["id"].as_i64()?, c["name"].as_str()?.to_string())))
        .collect();
    // Meilleur prix de vente récent par marchandise (et terminal correspondant).
    let now = crate::jeu::iso_vers_secondes(&crate::jeu::iso_maintenant()).unwrap_or(0) as f64;
    let mut meilleur: HashMap<i64, (f64, String)> = HashMap::new();
    for p in prix.as_array().into_iter().flatten() {
        let (Some(c), v) = (p["id_commodity"].as_i64(), nombre(&p["price_sell"])) else { continue };
        if v <= 0.0 || now - nombre(&p["date_modified"]) > AGE_PRIX_MAX_S {
            continue;
        }
        let t = p["terminal_name"].as_str().unwrap_or("").to_string();
        if meilleur.get(&c).is_none_or(|(x, _)| v > *x) {
            meilleur.insert(c, (v, t));
        }
    }
    let mut minerais: Vec<Value> = com
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| nombre(&c["is_raw"]) > 0.0 || nombre(&c["is_mineral"]) > 0.0)
        .filter_map(|c| {
            let id = c["id"].as_i64()?;
            let nom = c["name"].as_str()?.to_string();
            // Brut raffinable → version raffinée (id_parent) ; sinon il se vend tel quel.
            let raffine = (nombre(&c["is_refinable"]) > 0.0).then(|| c["id_parent"].as_i64()).flatten().filter(|p| *p > 0);
            let cible = raffine.unwrap_or(id);
            let (prix, terminal) = meilleur.get(&cible).cloned().unwrap_or((0.0, String::new()));
            Some(json!({
                "nom": nom,
                "raffine": raffine.and_then(|r| noms.get(&r).cloned()),
                "prixVente": prix,
                "terminalVente": terminal,
            }))
        })
        .collect();
    minerais.sort_by(|a, b| a["nom"].as_str().cmp(&b["nom"].as_str()));

    // Raffineries : bonus de rendement par minerai (valeur en %).
    let mut raff: HashMap<String, (String, Value)> = HashMap::new();
    for r in rend.as_array().into_iter().flatten() {
        let Some(t) = r["terminal_name"].as_str() else { continue };
        let e = raff.entry(t.to_string()).or_insert_with(|| (r["star_system_name"].as_str().unwrap_or("").to_string(), json!({})));
        if let Some(m) = r["commodity_name"].as_str() {
            e.1[m] = json!(nombre(&r["value"]));
        }
    }
    let mut raffineries: Vec<Value> =
        raff.into_iter().map(|(t, (s, b))| json!({ "nom": t, "systeme": s, "bonus": b })).collect();
    raffineries.sort_by(|a, b| (a["systeme"].as_str(), a["nom"].as_str()).cmp(&(b["systeme"].as_str(), b["nom"].as_str())));

    let methodes: Vec<Value> = meth
        .as_array()
        .into_iter()
        .flatten()
        .map(|m| json!({
            "nom": m["name"], "rendement": m["rating_yield"], "cout": m["rating_cost"], "vitesse": m["rating_speed"]
        }))
        .collect();
    Ok(json!({ "minerais": minerais, "raffineries": raffineries, "methodes": methodes, "prixRecusA": recu }))
}

// --- Sessions ---------------------------------------------------------------------------------------------

fn lire_sessions(m: &Minage) -> Vec<Session> {
    let c = m.conn.lock().unwrap();
    let Ok(mut req) = c.prepare(
        "SELECT id, debut, fin, vaisseau, lieu, raffinerie, methode, minerais, gains, notes, envoye FROM sessions ORDER BY debut DESC",
    ) else {
        return Vec::new();
    };
    let lignes = req.query_map([], |r| {
        let minerais: String = r.get(7)?;
        Ok(Session {
            id: r.get(0)?,
            debut: r.get(1)?,
            fin: r.get(2)?,
            vaisseau: r.get(3)?,
            lieu: r.get(4)?,
            raffinerie: r.get(5)?,
            methode: r.get(6)?,
            minerais: serde_json::from_str(&minerais).unwrap_or_default(),
            gains: r.get(8)?,
            notes: r.get(9)?,
            envoye: r.get::<_, i64>(10)? != 0,
        })
    });
    lignes.map(|it| it.filter_map(Result::ok).collect()).unwrap_or_default()
}

fn ecrire(m: &Minage, s: &Session) -> Result<(), String> {
    let c = m.conn.lock().unwrap();
    c.execute(
        "INSERT INTO sessions (id, debut, fin, vaisseau, lieu, raffinerie, methode, minerais, gains, notes, envoye)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(id) DO UPDATE SET debut=?2, fin=?3, vaisseau=?4, lieu=?5, raffinerie=?6, methode=?7,
           minerais=?8, gains=?9, notes=?10, envoye=?11",
        params![
            s.id, s.debut, s.fin, s.vaisseau, s.lieu, s.raffinerie, s.methode,
            serde_json::to_string(&s.minerais).unwrap_or_else(|_| "[]".into()),
            s.gains, s.notes, s.envoye as i64
        ],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Nettoie une session venue de l'interface (longueurs du contrat, valeurs positives).
fn nettoyer(mut s: Session) -> Result<Session, String> {
    let court = |o: Option<String>, n: usize| o.map(|v| v.trim().chars().take(n).collect::<String>()).filter(|v| !v.is_empty());
    s.vaisseau = court(s.vaisseau, 80);
    s.lieu = court(s.lieu, 120);
    s.raffinerie = court(s.raffinerie, 80);
    s.methode = court(s.methode, 40);
    s.notes = court(s.notes, 1000);
    s.minerais.retain(|m| !m.nom.trim().is_empty() && m.scu > 0.0);
    for m in &mut s.minerais {
        m.nom = m.nom.trim().chars().take(60).collect();
        m.scu = (m.scu * 100.0).round() / 100.0;
    }
    s.minerais.truncate(20);
    s.gains = s.gains.filter(|g| *g >= 0.0).map(|g| g.round());
    if crate::jeu::iso_vers_secondes(&s.debut).is_none() {
        return Err("Heure de début invalide.".into());
    }
    if let Some(f) = &s.fin {
        if crate::jeu::iso_vers_secondes(f) < crate::jeu::iso_vers_secondes(&s.debut) {
            return Err("La fin est avant le début.".into());
        }
    }
    Ok(s)
}

fn evenement(s: &Session) -> Option<Value> {
    let fin = s.fin.as_ref()?;
    let duree = crate::jeu::iso_vers_secondes(fin)? - crate::jeu::iso_vers_secondes(&s.debut)?;
    let mut d = json!({
        "startedAt": s.debut,
        "durationSec": duree.max(0),
        "ores": s.minerais.iter().map(|m| json!({ "name": m.nom, "scu": m.scu })).collect::<Vec<_>>(),
    });
    if let Some(r) = &s.raffinerie {
        d["refinery"] = json!(r);
    }
    if let Some(g) = s.gains {
        d["earningsAuec"] = json!(g as i64);
    }
    Some(d)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bilan {
    sessions: Vec<Session>,
    en_cours: Option<Session>,
    total_scu: f64,
    total_gains: f64,
    total_heures: f64,
    nb: usize,
}

fn bilan(m: &Minage) -> Bilan {
    let toutes = lire_sessions(m);
    let en_cours = toutes.iter().find(|s| s.fin.is_none()).cloned();
    let finies: Vec<Session> = toutes.into_iter().filter(|s| s.fin.is_some()).collect();
    let mut b = Bilan { nb: finies.len(), en_cours, total_scu: 0.0, total_gains: 0.0, total_heures: 0.0, sessions: Vec::new() };
    for s in &finies {
        b.total_scu += s.minerais.iter().map(|m| m.scu).sum::<f64>();
        b.total_gains += s.gains.unwrap_or(0.0);
        if let (Some(d), Some(f)) = (crate::jeu::iso_vers_secondes(&s.debut), s.fin.as_deref().and_then(crate::jeu::iso_vers_secondes)) {
            b.total_heures += (f - d).max(0) as f64 / 3600.0;
        }
    }
    b.sessions = finies;
    b
}

#[tauri::command]
pub fn minage_bilan(minage: tauri::State<'_, Minage>) -> Bilan {
    bilan(&minage)
}

#[tauri::command]
pub fn minage_commencer(minage: tauri::State<'_, Minage>, vaisseau: Option<String>) -> Result<Session, String> {
    if lire_sessions(&minage).iter().any(|s| s.fin.is_none()) {
        return Err("Une session est déjà en cours.".into());
    }
    let debut = crate::jeu::iso_maintenant();
    let s = nettoyer(Session {
        id: uuid::Uuid::new_v4().to_string(),
        debut,
        fin: None,
        vaisseau,
        lieu: None,
        raffinerie: None,
        methode: None,
        minerais: Vec::new(),
        gains: None,
        notes: None,
        envoye: false,
    })?;
    ecrire(&minage, &s)?;
    Ok(s)
}

/// Enregistre une session (brouillon ou corrigée). `terminer` : pose la fin maintenant (si absente) et la partage
/// si le réglage `mining` le permet. Renvoie un message pour l'interface.
#[tauri::command(async)]
pub fn minage_enregistrer(
    minage: tauri::State<'_, Minage>,
    portail: tauri::State<'_, Portail>,
    session: Session,
    terminer: Option<bool>,
) -> Result<String, String> {
    let mut s = nettoyer(session)?;
    let ancienne = lire_sessions(&minage).into_iter().find(|x| x.id == s.id);
    s.envoye = ancienne.as_ref().is_some_and(|a| a.envoye);
    if terminer.unwrap_or(false) && s.fin.is_none() {
        s.fin = Some(crate::jeu::iso_maintenant());
    }
    let mut message = if s.fin.is_some() { "Session enregistrée.".to_string() } else { "Brouillon enregistré.".to_string() };
    if s.fin.is_some() && !s.envoye {
        if let Some(data) = evenement(&s) {
            let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, format!("ces-compagnon|mining|{}", s.id).as_bytes()).to_string();
            match envoyer_evenement(&portail, &id, "mining.session", "mining", s.fin.as_deref().unwrap_or(&s.debut), &data) {
                Ok(true) => {
                    s.envoye = true;
                    message.push_str(" Elle sera partagée avec le portail.");
                }
                Ok(false) => message.push_str(" Gardée sur ce PC (partage du minage désactivé, ou compte non relié)."),
                Err(e) => message.push_str(&format!(" Partage impossible : {e}")),
            }
        }
    }
    ecrire(&minage, &s)?;
    Ok(message)
}

#[tauri::command]
pub fn minage_supprimer(minage: tauri::State<'_, Minage>, id: String) -> Result<(), String> {
    let c = minage.conn.lock().unwrap();
    c.execute("DELETE FROM sessions WHERE id = ?1", params![id]).map(|_| ()).map_err(|e| e.to_string())
}

fn csv(v: &str) -> String {
    if v.contains([';', '"', '\n']) { format!("\"{}\"", v.replace('"', "\"\"")) } else { v.to_string() }
}

#[tauri::command(async)]
pub fn minage_exporter(app: tauri::AppHandle, minage: tauri::State<'_, Minage>) -> Result<Option<String>, String> {
    let b = bilan(&minage);
    let mut t = String::from("\u{feff}debut;fin;duree_min;vaisseau;lieu;raffinerie;methode;minerais;scu_total;gains_auec;notes\r\n");
    for s in &b.sessions {
        let duree = match (crate::jeu::iso_vers_secondes(&s.debut), s.fin.as_deref().and_then(crate::jeu::iso_vers_secondes)) {
            (Some(d), Some(f)) => ((f - d) / 60).to_string(),
            _ => String::new(),
        };
        let minerais = s.minerais.iter().map(|m| format!("{} {}", m.nom, m.scu)).collect::<Vec<_>>().join(", ");
        let scu: f64 = s.minerais.iter().map(|m| m.scu).sum();
        let champs = [
            s.debut.clone(),
            s.fin.clone().unwrap_or_default(),
            duree,
            s.vaisseau.clone().unwrap_or_default(),
            s.lieu.clone().unwrap_or_default(),
            s.raffinerie.clone().unwrap_or_default(),
            s.methode.clone().unwrap_or_default(),
            minerais,
            scu.to_string().replace('.', ","),
            s.gains.map(|g| g.to_string()).unwrap_or_default(),
            s.notes.clone().unwrap_or_default(),
        ];
        t.push_str(&champs.iter().map(|c| csv(c)).collect::<Vec<_>>().join(";"));
        t.push_str("\r\n");
    }
    let Some(cible) = app
        .dialog()
        .file()
        .set_title("Exporter les sessions de minage")
        .set_file_name("minage-ces.csv")
        .add_filter("Tableur (CSV)", &["csv"])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let chemin = cible.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&chemin, t).map_err(|e| e.to_string())?;
    Ok(Some(chemin.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session {
            id: "s1".into(),
            debut: "2026-11-15T10:00:00.000Z".into(),
            fin: Some("2026-11-15T11:30:00.000Z".into()),
            vaisseau: Some("  Prospector  ".into()),
            lieu: None,
            raffinerie: Some("CRU-L1".into()),
            methode: None,
            minerais: vec![Minerai { nom: "Quantainium (Raw)".into(), scu: 12.345 }, Minerai { nom: "".into(), scu: 3.0 }],
            gains: Some(123456.7),
            notes: None,
            envoye: false,
        }
    }

    #[test]
    fn nettoyage_et_evenement_du_contrat() {
        let s = nettoyer(session()).unwrap();
        assert_eq!(s.vaisseau.as_deref(), Some("Prospector"));
        assert_eq!(s.minerais.len(), 1);
        assert_eq!(s.minerais[0].scu, 12.35);
        let e = evenement(&s).unwrap();
        assert_eq!(e["durationSec"], 5400);
        assert_eq!(e["ores"][0]["name"], "Quantainium (Raw)");
        assert_eq!(e["refinery"], "CRU-L1");
        assert_eq!(e["earningsAuec"], 123457);
        assert!(e.get("vaisseau").is_none(), "seuls les champs du contrat partent");
    }

    #[test]
    fn stockage_et_bilan() {
        let d = Dossiers { donnees: std::env::temp_dir().join(format!("cmp-minage-{}", std::process::id())) };
        let _ = std::fs::remove_dir_all(&d.donnees);
        let m = Minage::ouvrir(&d).unwrap();
        ecrire(&m, &nettoyer(session()).unwrap()).unwrap();
        let mut en_cours = nettoyer(session()).unwrap();
        en_cours.id = "s2".into();
        en_cours.fin = None;
        ecrire(&m, &en_cours).unwrap();
        let b = bilan(&m);
        assert_eq!(b.nb, 1);
        assert_eq!(b.en_cours.map(|s| s.id), Some("s2".into()));
        assert!((b.total_heures - 1.5).abs() < 1e-9);
        assert_eq!(b.total_gains, 123457.0);
        let mut faux = session();
        faux.fin = Some("2026-11-15T09:00:00.000Z".into());
        assert!(nettoyer(faux).is_err());
    }
}
