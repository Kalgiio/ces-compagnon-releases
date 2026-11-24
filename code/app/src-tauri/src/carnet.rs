//! Carnet de voyage détaillé (feuille de route § 6) : chaque session de jeu et ses étapes, gardées sur le PC
//! (SQLite `carnet.db`, 90 jours). Le journal du jeu est relu au démarrage : l'enregistrement est idempotent
//! (clé = session + position de la ligne + type).

use rusqlite::{params, Connection};
use sclog::{Evenement, Presence};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;

const RETENTION_JOURS: i64 = 90;

pub struct Carnet {
    conn: Mutex<Connection>,
}

/// Texte français d'une étape (et son type), ou `None` si l'événement n'a pas sa place dans le carnet.
fn decrire(e: &Evenement) -> Option<(&'static str, String)> {
    Some(match e {
        Evenement::SessionDebut { .. } => ("session.start", "Session de jeu commencée".into()),
        Evenement::SessionFin { at, started_at, .. } => {
            let duree = started_at
                .as_deref()
                .and_then(crate::jeu::iso_vers_secondes)
                .zip(crate::jeu::iso_vers_secondes(at))
                .map(|(d, f)| (f - d).max(0) / 60);
            let texte = match duree {
                Some(m) if m >= 60 => format!("Session terminée ({} h {:02})", m / 60, m % 60),
                Some(m) => format!("Session terminée ({m} min)"),
                None => "Session terminée".into(),
            };
            ("session.end", texte)
        }
        Evenement::Presence { state: Presence::Universe, region, .. } => (
            "presence.universe",
            match region.as_deref() {
                Some("euw") => "Univers rejoint (serveur Europe de l'Ouest)".into(),
                Some(r) => format!("Univers rejoint (serveur {r})"),
                None => "Univers rejoint".into(),
            },
        ),
        Evenement::Presence { state: Presence::Menu, .. } => ("presence.menu", "Au menu principal".into()),
        Evenement::Presence { .. } => return None,
        Evenement::RouteCalculee { depart, destination, .. } => ("travel.route", format!("Route tracée : {depart} → {destination}")),
        Evenement::SautQuantique { vehicle_class, destination, .. } => {
            let v = sclog::vaisseaux::nom_vaisseau(vehicle_class);
            (
                "travel.quantum",
                match destination {
                    Some(d) => format!("Saut quantique terminé → {d} ({v})"),
                    None => format!("Saut quantique terminé ({v})"),
                },
            )
        }
        Evenement::Juridiction { nom, .. } => ("travel.jurisdiction", format!("Juridiction : {nom}")),
        Evenement::Armistice { entree: true, .. } => ("travel.armistice", "Entrée en zone d'armistice".into()),
        Evenement::Armistice { entree: false, .. } => ("travel.armistice", "Sortie de zone d'armistice".into()),
        Evenement::JoueurATerre { kind, .. } => {
            ("player.down", if kind == "dead" { "Mort (réveil en lit médical)".into() } else { "À terre (incapacité)".into() })
        }
        Evenement::VaisseauDetruit { vehicle_class, role, .. } => {
            let v = sclog::vaisseaux::nom_vaisseau(vehicle_class);
            (
                "vehicle.destroyed",
                match role.as_str() {
                    "killer" => format!("Vaisseau détruit par toi : {v}"),
                    "both" => format!("Ton vaisseau détruit par toi : {v}"),
                    _ => format!("Ton vaisseau détruit : {v}"),
                },
            )
        }
        Evenement::MissionTerminee { .. } => ("mission.completed", "Mission réussie".into()),
    })
}

fn decalage(e: &Evenement) -> i64 {
    let d = match e {
        Evenement::SessionDebut { decalage, .. }
        | Evenement::SessionFin { decalage, .. }
        | Evenement::Presence { decalage, .. }
        | Evenement::VaisseauDetruit { decalage, .. }
        | Evenement::JoueurATerre { decalage, .. }
        | Evenement::SautQuantique { decalage, .. }
        | Evenement::RouteCalculee { decalage, .. }
        | Evenement::Juridiction { decalage, .. }
        | Evenement::Armistice { decalage, .. }
        | Evenement::MissionTerminee { decalage, .. } => *decalage,
    };
    d.min(i64::MAX as u64) as i64
}

impl Carnet {
    pub fn ouvrir(dossier: &Path) -> rusqlite::Result<Carnet> {
        let _ = std::fs::create_dir_all(dossier);
        let conn = Connection::open(dossier.join("carnet.db"))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS etapes (
               session TEXT NOT NULL,
               at TEXT NOT NULL,
               type TEXT NOT NULL,
               texte TEXT NOT NULL,
               decalage INTEGER NOT NULL,
               PRIMARY KEY (session, decalage, type)
             );
             CREATE INDEX IF NOT EXISTS etapes_at ON etapes(at);",
        )?;
        let limite = crate::jeu::iso_depuis(
            crate::jeu::iso_vers_secondes(&crate::jeu::iso_maintenant()).unwrap_or(0) - RETENTION_JOURS * 86_400,
            0,
        );
        let _ = conn.execute("DELETE FROM etapes WHERE at < ?1", params![limite]);
        Ok(Carnet { conn: Mutex::new(conn) })
    }

    pub fn enregistrer(&self, session: &str, e: &Evenement) {
        let Some((type_, texte)) = decrire(e) else { return };
        let c = self.conn.lock().unwrap();
        let _ = c.execute(
            "INSERT INTO etapes (session, at, type, texte, decalage) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(session, decalage, type) DO UPDATE SET texte = excluded.texte",
            params![session, e.at(), type_, texte, decalage(e)],
        );
    }

    fn sessions(&self) -> Vec<ResumeSession> {
        let c = self.conn.lock().unwrap();
        let Ok(mut req) = c.prepare(
            "SELECT session,
                    MAX(at),
                    SUM(type = 'travel.quantum'),
                    SUM(type = 'mission.completed'),
                    SUM(type = 'player.down'),
                    SUM(type = 'vehicle.destroyed'),
                    MAX(type = 'session.end'),
                    COUNT(*)
             FROM etapes GROUP BY session ORDER BY session DESC LIMIT 200",
        ) else {
            return Vec::new();
        };
        let lignes = req.query_map([], |r| {
            Ok(ResumeSession {
                session: r.get(0)?,
                derniere: r.get(1)?,
                sauts: r.get(2)?,
                missions: r.get(3)?,
                a_terre: r.get(4)?,
                vaisseaux: r.get(5)?,
                terminee: r.get::<_, i64>(6)? != 0,
                etapes: r.get(7)?,
                juridictions: Vec::new(),
            })
        });
        let mut v: Vec<ResumeSession> = lignes.map(|it| it.filter_map(Result::ok).collect()).unwrap_or_default();
        for s in &mut v {
            if let Ok(mut q) = c.prepare("SELECT DISTINCT trim(substr(texte, 14)) FROM etapes WHERE session = ?1 AND type = 'travel.jurisdiction'") {
                s.juridictions = q.query_map(params![s.session], |r| r.get::<_, String>(0)).map(|it| it.filter_map(Result::ok).collect()).unwrap_or_default();
            }
        }
        v
    }

    fn etapes(&self, session: &str) -> Vec<Etape> {
        let c = self.conn.lock().unwrap();
        let Ok(mut req) = c.prepare("SELECT at, type, texte FROM etapes WHERE session = ?1 ORDER BY at, decalage") else {
            return Vec::new();
        };
        req.query_map(params![session], |r| Ok(Etape { at: r.get(0)?, type_: r.get(1)?, texte: r.get(2)? }))
            .map(|it| it.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    /// Lieux atteints (destinations des sauts quantiques), du plus ancien au plus récent : carte stellaire du carnet.
    fn lieux(&self) -> Vec<Lieu> {
        let c = self.conn.lock().unwrap();
        let Ok(mut req) = c.prepare("SELECT at, texte, session FROM etapes WHERE type = 'travel.quantum' ORDER BY at, decalage") else {
            return Vec::new();
        };
        req.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
            .map(|it| {
                it.filter_map(Result::ok)
                    .filter_map(|(at, texte, session)| destination(&texte).map(|location| Lieu { location, kind: "quantum", at, session }))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// « Saut quantique terminé → Clio (RSI_Perseus) » → « Clio ».
fn destination(texte: &str) -> Option<String> {
    let reste = texte.split_once(" → ")?.1;
    let nom = match reste.rfind(" (") {
        Some(i) if reste.ends_with(')') => &reste[..i],
        _ => reste,
    };
    let nom = nom.trim();
    (!nom.is_empty()).then(|| nom.to_string())
}

#[derive(Serialize)]
pub struct Lieu {
    location: String,
    kind: &'static str,
    at: String,
    session: String,
}

#[tauri::command]
pub fn carnet_lieux(carnet: tauri::State<'_, std::sync::Arc<Carnet>>) -> Vec<Lieu> {
    carnet.lieux()
}

#[cfg(test)]
mod tests_lieux {
    use super::destination;

    #[test]
    fn destinations() {
        assert_eq!(destination("Saut quantique terminé → Clio (RSI_Perseus)").as_deref(), Some("Clio"));
        assert_eq!(destination("Saut quantique terminé → Pyro V – L2 (DRAK_Ironclad_Assault)").as_deref(), Some("Pyro V – L2"));
        assert_eq!(destination("Saut quantique terminé (RSI_Polaris)"), None);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeSession {
    /// Heure de début de la session (« Log started » du journal) : identifiant de la session.
    session: String,
    derniere: String,
    sauts: i64,
    missions: i64,
    a_terre: i64,
    vaisseaux: i64,
    terminee: bool,
    etapes: i64,
    juridictions: Vec<String>,
}

#[derive(Serialize)]
pub struct Etape {
    at: String,
    #[serde(rename = "type")]
    type_: String,
    texte: String,
}

#[tauri::command]
pub fn carnet_sessions(carnet: tauri::State<'_, std::sync::Arc<Carnet>>) -> Vec<ResumeSession> {
    carnet.sessions()
}

#[tauri::command]
pub fn carnet_session(carnet: tauri::State<'_, std::sync::Arc<Carnet>>, session: String) -> Vec<Etape> {
    carnet.etapes(&session)
}


/// Relit les anciens journaux (`logbackups\Game*.log`, LECTURE SEULE) pour remplir le carnet local.
/// Rien n'est envoyé au portail : ces événements ne passent pas par la file d'envoi.
#[tauri::command(async)]
pub fn carnet_importer(
    carnet: tauri::State<'_, std::sync::Arc<Carnet>>,
    surveillance: tauri::State<'_, crate::jeu::Surveillance>,
) -> Result<String, String> {
    let dossier = surveillance.etat.lock().unwrap().dossier_jeu.clone().ok_or("Dossier du jeu introuvable.")?;
    let sauvegardes = std::path::Path::new(&dossier).join("logbackups");
    let mut fichiers: Vec<std::path::PathBuf> = std::fs::read_dir(&sauvegardes)
        .map_err(|_| "Aucun ancien journal (dossier logbackups introuvable).".to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().is_some_and(|n| {
            let n = n.to_string_lossy();
            n.starts_with("Game") && n.ends_with(".log")
        }))
        .collect();
    fichiers.sort();
    let mut sessions = 0;
    for f in &fichiers {
        let mut s = sclog::Suiveur::nouveau(f);
        let mut a = sclog::Analyseur::nouveau();
        let mut vue = false;
        loop {
            let l = s.lire().map_err(|e| e.to_string())?;
            for (pos, ligne) in &l.lignes {
                for ev in a.traiter(*pos, ligne) {
                    let session = match &ev {
                        Evenement::SessionFin { started_at, .. } => started_at.clone(),
                        _ => a.debut_session().map(str::to_string),
                    };
                    if let Some(session) = session {
                        carnet.enregistrer(&session, &ev);
                        vue = true;
                    }
                }
            }
            if !l.suite {
                break;
            }
        }
        if vue {
            sessions += 1;
        }
    }
    Ok(format!("{sessions} session(s) importée(s) depuis {} ancien(s) journal(aux).", fichiers.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idempotent_et_resume() {
        let d = std::env::temp_dir().join(format!("cmp-carnet-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let c = Carnet::ouvrir(&d).unwrap();
        let s = "2026-11-15T10:00:00.000Z";
        let evs = vec![
            Evenement::SessionDebut { at: s.into(), decalage: 0 },
            Evenement::Juridiction { at: "2026-11-15T10:05:00.000Z".into(), decalage: 10, nom: "microTech".into() },
            Evenement::SautQuantique { at: "2026-11-15T10:10:00.000Z".into(), decalage: 20, vehicle_class: "DRAK_Ironclad".into(), destination: Some("Clio".into()) },
            Evenement::SessionFin { at: "2026-11-15T11:40:00.000Z".into(), decalage: 30, started_at: Some(s.into()) },
        ];
        for _ in 0..2 {
            for e in &evs {
                c.enregistrer(s, e); // relu deux fois : aucun doublon
            }
        }
        let r = c.sessions();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].etapes, 4);
        assert_eq!(r[0].sauts, 1);
        assert!(r[0].terminee);
        assert_eq!(r[0].juridictions, vec!["microTech".to_string()]);
        let e = c.etapes(s);
        assert_eq!(e[2].texte, "Saut quantique terminé → Clio (Drake Ironclad)");
        assert_eq!(e[3].texte, "Session terminée (1 h 40)");
    }
}
