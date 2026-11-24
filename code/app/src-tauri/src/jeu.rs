//! Le jeu vu de l'extérieur : processus lancé ou non, dossier d'installation, `Game.log` suivi en direct.
//!
//! Une tâche de fond lit le journal toutes les secondes (lecture partagée : le jeu continue d'écrire) et tient
//! l'état à jour. L'interface le lit par la commande `etat`.

use sclog::{Analyseur, Evenement, Presence, Suiveur};
use serde::Serialize;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{ProcessesToUpdate, System};

const PROCESSUS_JEU: &str = "StarCitizen.exe";
const EVENEMENTS_GARDES: usize = 200;

/// Un événement vu, tel que l'interface l'affiche.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvenementVu {
    #[serde(flatten)]
    pub evenement: Evenement,
    /// Lu au démarrage de l'application dans un journal déjà écrit (session passée ou en cours).
    pub historique: bool,
}

#[derive(Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EtatJeu {
    pub jeu_lance: bool,
    /// Présence combinée : processus + journal (`offline` si le jeu n'est pas lancé).
    pub presence: Presence,
    /// Depuis quand (ISO UTC) : heure de la ligne du journal, ou constat du processus.
    pub presence_depuis: Option<String>,
    pub region: Option<String>,
    pub session_debut: Option<String>,
    pub build: Option<String>,
    pub version: Option<String>,
    pub dossier_jeu: Option<String>,
    pub dossier_manuel: bool,
    pub journal_trouve: bool,
    pub journal_taille_mo: Option<u64>,
    pub evenements: VecDeque<EvenementVu>,
}

/// Un événement transmis à la tâche portail, avec le contexte de sa session.
#[derive(Clone)]
pub struct EvenementBrut {
    pub evenement: Evenement,
    pub session_debut: Option<String>,
    pub version: Option<String>,
    pub build: Option<String>,
}

pub struct Surveillance {
    pub etat: Arc<Mutex<EtatJeu>>,
    /// Dossier choisi à la main (Paramètres) ; `None` = détection automatique. Lu par la tâche de fond.
    pub dossier_force: Arc<Mutex<Option<PathBuf>>>,
    pub fichier_dossier: PathBuf,
    /// Boîte d'envoi : la tâche portail la vide (filtre, file locale, envoi).
    pub sortie: Arc<Mutex<Vec<EvenementBrut>>>,
}

/// Cherche `…\StarCitizen\LIVE` aux emplacements habituels, sur chaque lecteur.
fn trouver_dossier_jeu() -> Option<PathBuf> {
    const SOUS_CHEMINS: &[&str] = &[
        r"RSI\StarCitizen\LIVE",
        r"Roberts Space Industries\StarCitizen\LIVE",
        r"Program Files\Roberts Space Industries\StarCitizen\LIVE",
        r"Games\RSI\StarCitizen\LIVE",
        r"Games\Roberts Space Industries\StarCitizen\LIVE",
    ];
    for lettre in b'C'..=b'Z' {
        let racine = format!("{}:\\", lettre as char);
        if !Path::new(&racine).exists() {
            continue;
        }
        for sc in SOUS_CHEMINS {
            let p = Path::new(&racine).join(sc);
            if p.join("Bin64").is_dir() || p.join("Game.log").is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Lance la tâche de fond et renvoie l'état partagé.
pub fn demarrer(carnet: Arc<crate::carnet::Carnet>, donnees: &Path) -> Surveillance {
    let partage = Arc::new(Mutex::new(EtatJeu::default()));
    let sortie = Arc::new(Mutex::new(Vec::new()));
    let fichier_dossier = donnees.join("dossier-jeu.json");
    let force = std::fs::read(&fichier_dossier)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v["dossier"].as_str().map(PathBuf::from))
        .filter(|d| est_dossier_jeu(d));
    let dossier_force = Arc::new(Mutex::new(force));
    let (p, s, f) = (partage.clone(), sortie.clone(), dossier_force.clone());
    std::thread::Builder::new()
        .name("surveillance-jeu".into())
        .spawn(move || boucle(p, s, carnet, f))
        .expect("tâche de surveillance");
    Surveillance { etat: partage, sortie, dossier_force, fichier_dossier }
}

fn boucle(
    partage: Arc<Mutex<EtatJeu>>,
    sortie: Arc<Mutex<Vec<EvenementBrut>>>,
    carnet: Arc<crate::carnet::Carnet>,
    dossier_force: Arc<Mutex<Option<PathBuf>>>,
) {
    let mut systeme = System::new();
    let mut force_vu = dossier_force.lock().unwrap().clone();
    let mut dossier = force_vu.clone().or_else(trouver_dossier_jeu);
    let mut dernier_scan = Instant::now();
    let mut suiveur: Option<Suiveur> = dossier.as_ref().map(|d| Suiveur::nouveau(d.join("Game.log")));
    let mut analyseur = Analyseur::nouveau();
    let mut rattrapage = true; // premier passage : on lit ce qui est déjà écrit
    let mut presence_journal_at: Option<String> = None;
    let mut jeu_lance_avant = false;

    loop {
        // Dossier choisi (ou rendu à la détection automatique) depuis l'interface : on repart sur ce dossier.
        let force = dossier_force.lock().unwrap().clone();
        if force != force_vu {
            force_vu = force.clone();
            dossier = force.or_else(trouver_dossier_jeu);
            suiveur = dossier.as_ref().map(|d| Suiveur::nouveau(d.join("Game.log")));
            analyseur = Analyseur::nouveau();
            rattrapage = true;
            dernier_scan = Instant::now();
        }
        // Dossier du jeu (re)cherché toutes les 30 s tant qu'il est introuvable.
        if dossier.is_none() && dernier_scan.elapsed() > Duration::from_secs(30) {
            dossier = trouver_dossier_jeu();
            dernier_scan = Instant::now();
            suiveur = dossier.as_ref().map(|d| Suiveur::nouveau(d.join("Game.log")));
        }

        systeme.refresh_processes(ProcessesToUpdate::All, true);
        let jeu_lance = systeme.processes_by_exact_name(PROCESSUS_JEU.as_ref()).next().is_some();

        let mut nouveaux = Vec::new();
        let mut rotation = false;
        if let Some(s) = suiveur.as_mut() {
            loop {
                match s.lire() {
                    Ok(l) => {
                        if l.nouveau_fichier {
                            analyseur.nouveau_fichier();
                            rotation = true;
                        }
                        for (pos, ligne) in &l.lignes {
                            for ev in analyseur.traiter(*pos, ligne) {
                                if let Evenement::Presence { at, .. } = &ev {
                                    presence_journal_at = Some(at.clone());
                                }
                                nouveaux.push(ev);
                            }
                        }
                        if !l.suite {
                            break;
                        }
                    }
                    Err(_) => break, // fichier momentanément inaccessible : on réessaiera
                }
            }
        }

        // Le jeu a disparu sans <SystemQuit> (plantage, fin de tâche) : on clôt la session ouverte.
        if jeu_lance_avant && !jeu_lance && analyseur.debut_session().is_some() {
            nouveaux.extend(analyseur.fermer_session(&iso_maintenant()));
        }
        jeu_lance_avant = jeu_lance;
        if !nouveaux.is_empty() {
            let mut s = sortie.lock().expect("boîte d'envoi");
            for ev in &nouveaux {
                let session_debut = match ev {
                    Evenement::SessionFin { started_at, .. } => started_at.clone(),
                    _ => analyseur.debut_session().map(str::to_string),
                };
                if let Some(session) = &session_debut {
                    carnet.enregistrer(session, ev);
                }
                s.push(EvenementBrut {
                    evenement: ev.clone(),
                    session_debut,
                    version: analyseur.version().map(str::to_string),
                    build: analyseur.build().map(str::to_string),
                });
            }
        }

        let taille = suiveur.as_ref().and_then(|s| std::fs::metadata(s.chemin()).ok()).map(|m| m.len());
        {
            let mut e = partage.lock().expect("verrou de l'état");
            e.jeu_lance = jeu_lance;
            let journal = analyseur.presence();
            let combinee = match (jeu_lance, journal) {
                (false, _) => Presence::Offline,
                (true, Presence::Offline) => Presence::Launching,
                (true, p) => p,
            };
            if combinee != e.presence || e.presence_depuis.is_none() {
                // Heure de la ligne du journal si c'est lui qui donne l'état, sinon maintenant (processus).
                e.presence_depuis = if combinee == journal && jeu_lance {
                    presence_journal_at.clone().or_else(|| Some(iso_maintenant()))
                } else {
                    Some(iso_maintenant())
                };
            }
            e.presence = combinee;
            e.region = if e.presence == Presence::Universe { analyseur.region().map(str::to_string) } else { None };
            e.session_debut = analyseur.debut_session().map(str::to_string);
            e.build = analyseur.build().map(str::to_string);
            e.version = analyseur.version().map(str::to_string);
            e.dossier_jeu = dossier.as_ref().map(|d| d.display().to_string());
            e.dossier_manuel = force_vu.is_some();
            e.journal_trouve = taille.is_some();
            e.journal_taille_mo = taille.map(|t| (t + 524_288) / 1_048_576);
            if rotation && !rattrapage {
                e.evenements.clear();
            }
            for ev in nouveaux {
                e.evenements.push_front(EvenementVu { evenement: ev, historique: rattrapage });
            }
            e.evenements.truncate(EVENEMENTS_GARDES);
        }
        rattrapage = false;
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Heure actuelle en ISO 8601 UTC (`2026-11-14T20:12:03.512Z`), sans dépendance.
pub fn iso_maintenant() -> String {
    let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    iso_depuis(d.as_secs() as i64, d.subsec_millis())
}

/// Secondes depuis 1970 (+ millisecondes) → ISO 8601 UTC.
pub fn iso_depuis(secs: i64, millis: u32) -> String {
    let (jours, reste) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // jours → date civile (algorithme de H. Hinnant)
    let z = jours + 719_468;
    let ere = z.div_euclid(146_097);
    let doe = z - ere * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let jour = doy - (153 * mp + 2) / 5 + 1;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 };
    let an = yoe + ere * 400 + if mois <= 2 { 1 } else { 0 };
    format!(
        "{an:04}-{mois:02}-{jour:02}T{:02}:{:02}:{:02}.{:03}Z",
        reste / 3600,
        reste % 3600 / 60,
        reste % 60,
        millis
    )
}

/// ISO 8601 UTC → secondes depuis 1970 (pour les durées). `None` si le format n'est pas reconnu.
pub fn iso_vers_secondes(iso: &str) -> Option<i64> {
    let n = |a: usize, z: usize| iso.get(a..z)?.parse::<i64>().ok();
    let (an, mois, jour) = (n(0, 4)?, n(5, 7)?, n(8, 10)?);
    let (h, mi, s) = (n(11, 13)?, n(14, 16)?, n(17, 19)?);
    // date civile → jours (algorithme de H. Hinnant)
    let a = if mois <= 2 { an - 1 } else { an };
    let ere = a.div_euclid(400);
    let yoe = a - ere * 400;
    let doy = (153 * (if mois > 2 { mois - 3 } else { mois + 9 }) + 2) / 5 + jour - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let jours = ere * 146_097 + doe - 719_468;
    Some(jours * 86_400 + h * 3600 + mi * 60 + s)
}

#[tauri::command]
pub fn etat(surveillance: tauri::State<'_, Surveillance>) -> EtatJeu {
    surveillance.etat.lock().expect("verrou de l'état").clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_aller_retour() {
        // 2026-11-14T20:16:09Z = 1 794 687 369 s (vérifié avec Python datetime)
        assert_eq!(iso_vers_secondes("2026-11-14T20:16:09.267Z"), Some(1_794_687_369));
        assert_eq!(iso_depuis(1_794_687_369, 267), "2026-11-14T20:16:09.267Z");
        for s in [0i64, 951_782_400 /* 2000-02-29 */, 1_709_164_800 /* 2024-02-29 */, 1_794_687_369] {
            assert_eq!(iso_vers_secondes(&iso_depuis(s, 0)), Some(s));
        }
        assert_eq!(iso_depuis(951_782_400, 0), "2000-02-29T00:00:00.000Z");
        assert!(iso_maintenant().starts_with("20"));
    }
}

/// Un dossier d'installation de Star Citizen (LIVE, PTU…) : il contient `Bin64`, `Data.p4k` ou `Game.log`.
pub fn est_dossier_jeu(d: &Path) -> bool {
    d.join("Bin64").is_dir() || d.join("Data.p4k").is_file() || d.join("Game.log").is_file()
}

/// Dossier choisi par le membre → dossier d'installation retenu (accepte aussi le dossier `StarCitizen` parent).
pub fn resoudre_dossier(choisi: &Path) -> Option<PathBuf> {
    if est_dossier_jeu(choisi) {
        return Some(choisi.to_path_buf());
    }
    ["LIVE", "PTU", "EPTU", "TECH-PREVIEW"].iter().map(|s| choisi.join(s)).find(|d| est_dossier_jeu(d))
}

#[tauri::command(async)]
pub fn jeu_choisir_dossier(app: tauri::AppHandle, surveillance: tauri::State<'_, Surveillance>) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let Some(choix) = app.dialog().file().set_title("Dossier de Star Citizen (LIVE ou PTU)").blocking_pick_folder() else {
        return Ok(None); // annulé
    };
    let choisi = choix.into_path().map_err(|e| e.to_string())?;
    let dossier = resoudre_dossier(&choisi).ok_or_else(|| {
        format!(
            "« {} » n'est pas un dossier de Star Citizen. Choisis le dossier LIVE (ou PTU), qui contient Bin64 et Data.p4k.",
            choisi.display()
        )
    })?;
    crate::traduction::ecrire_sur(
        &surveillance.fichier_dossier,
        serde_json::json!({ "dossier": dossier.display().to_string() }).to_string().as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    *surveillance.dossier_force.lock().unwrap() = Some(dossier.clone());
    Ok(Some(dossier.display().to_string()))
}

#[tauri::command]
pub fn jeu_dossier_auto(surveillance: tauri::State<'_, Surveillance>) {
    let _ = std::fs::remove_file(&surveillance.fichier_dossier);
    *surveillance.dossier_force.lock().unwrap() = None;
}

#[cfg(test)]
mod tests_dossier {
    use super::*;

    #[test]
    fn dossier_parent_ou_live() {
        let base = std::env::temp_dir().join(format!("cmp-dossier-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let ptu = base.join("StarCitizen").join("PTU");
        std::fs::create_dir_all(ptu.join("Bin64")).unwrap();
        assert_eq!(resoudre_dossier(&base.join("StarCitizen")), Some(ptu.clone()));
        assert_eq!(resoudre_dossier(&ptu), Some(ptu.clone()));
        assert_eq!(resoudre_dossier(&base), None);
    }
}
