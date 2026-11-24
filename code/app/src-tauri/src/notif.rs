//! Notifications Windows et bandeau DEFCON (demandes du CES, 0.11.0).
//!
//! - Source : le direct du portail (`GET /api/chat/poll`, long polling 25 s), lu par le cœur avec le jeton, et
//!   `GET /api/site/defcon` au démarrage. Le portail ne publie `companion:launch` que pour les membres qui ont activé
//!   « Annoncer le lancement du jeu » ; aucun nom ne vient du jeu.
//! - Notifications : toasts natifs de Windows (comme Steam), avatar du membre en vignette ronde. Réglables, désactivables.
//! - DEFCON 3 → 1 : bandeau et liseré animé DANS la fenêtre de l'application (demande du CES, 0.12.0 : plus de
//!   fenêtre plein écran). L'interface lit l'état par `notif_defcon`.

use crate::portail::ORIGINE;
use crate::presence::{jeton_actuel, Portail};
use crate::traduction::Dossiers;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tauri::{Emitter, EventTarget, Manager};


#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Reglages {
    /// « X a lancé Star Citizen ».
    pub lancements: bool,
    /// Changement de niveau DEFCON.
    pub defcon: bool,
    /// Bandeau et liseré d'alerte dans la fenêtre, DEFCON 3 à 1.
    pub bandeau: bool,
    /// Notifications du portail (inscriptions, mentions, invitations…) en notifications Windows. Faux = sourdine.
    pub portail: bool,
    /// Discord Rich Presence pendant que le jeu tourne.
    pub discord: bool,
    /// Lancements et SOS : carte CES en bas à droite du bureau (annonce.rs) au lieu de la notification Windows.
    pub carte: bool,
    /// SOS au clavier : Ctrl + Maj + 1…6, deux appuis (raccourcis.rs). Faux par défaut.
    pub raccourcis_sos: bool,
}

impl Default for Reglages {
    fn default() -> Self {
        Reglages { lancements: true, defcon: true, bandeau: true, portail: true, discord: true, carte: true, raccourcis_sos: false }
    }
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EtatDefcon {
    pub niveau: u8,
    pub nom: Option<String>,
    pub message: Option<String>,
    /// Bandeau et liseré à afficher dans la fenêtre.
    pub bandeau: bool,
    /// Bandeau masqué par le membre pour ce niveau (réaffiché au prochain changement).
    pub masque: bool,
}

impl Notifications {
    /// Rich Presence voulue par le membre (réglage `discord`).
    /// SOS au clavier activé ?
    pub fn raccourcis_sos(&self) -> bool {
        self.reglages.lock().unwrap().raccourcis_sos
    }

    pub fn discord_actif(&self) -> bool {
        self.reglages.lock().unwrap().discord
    }
}

pub struct Notifications {
    reglages: Arc<Mutex<Reglages>>,
    defcon: Arc<Mutex<EtatDefcon>>,
    fichier: PathBuf,
    /// Notifications du portail déjà vues (direct `notification:new` et relevé périodique) : jamais deux toasts.
    vues: Arc<Mutex<std::collections::HashSet<String>>>,
}

fn id_texte(v: &Value) -> String {
    v.as_str().map(str::to_string).or_else(|| v.as_u64().map(|i| i.to_string())).unwrap_or_default()
}

fn lire_reglages(f: &Path) -> Reglages {
    std::fs::read(f).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn client_direct() -> &'static reqwest::blocking::Client {
    static C: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .user_agent(format!("CES-Compagnon/{} (Windows)", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(45)) // le portail répond au plus tard après 25 s
            .build()
            .expect("client HTTPS")
    })
}

fn get(jeton: &str, chemin: &str) -> Option<Value> {
    let r = client_direct().get(format!("{ORIGINE}{chemin}")).bearer_auth(jeton).send().ok()?;
    if !r.status().is_success() {
        return None;
    }
    r.json().ok()
}

// --- Toasts Windows -------------------------------------------------------------------------------------------

fn echapper(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Identifiant d'application (AUMID) : celui que l'installeur pose sur le raccourci du menu Démarrer (identifiant
/// Tauri). En développement, l'application n'a pas de raccourci : on emprunte celui de PowerShell pour pouvoir tester.
fn aumid() -> &'static str {
    if cfg!(debug_assertions) {
        "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe"
    } else {
        "fr.ces-sc.compagnon"
    }
}

pub fn toast(titre: &str, texte: &str, image: Option<&Path>) {
    use windows::core::HSTRING;
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    let vignette = image
        .map(|p| format!(r#"<image placement="appLogoOverride" hint-crop="circle" src="{}"/>"#, echapper(&p.to_string_lossy())))
        .unwrap_or_default();
    let xml = format!(
        r#"<toast><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text>{vignette}</binding></visual></toast>"#,
        echapper(titre),
        echapper(texte)
    );
    let r: windows::core::Result<()> = (|| {
        let doc = XmlDocument::new()?;
        doc.LoadXml(&HSTRING::from(xml))?;
        let n = ToastNotification::CreateToastNotification(&doc)?;
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(aumid()))?.Show(&n)
    })();
    if let Err(e) = r {
        eprintln!("[notif] toast impossible : {e}");
    }
}

/// Avatar du membre en PNG 96 px dans le cache de l'application (les toasts ne lisent ni WebP ni les URL distantes).
fn avatar(jeton: &str, cache: &Path, user_id: &str, url: Option<&str>) -> Option<PathBuf> {
    if user_id.is_empty() || !user_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    let fichier = cache.join(format!("{user_id}.png"));
    if fichier.metadata().ok().and_then(|m| m.modified().ok()).is_some_and(|t| t.elapsed().map_or(false, |d| d < Duration::from_secs(86_400))) {
        return Some(fichier);
    }
    let url = match url {
        Some(u) => u.to_string(),
        None => {
            let m = get(jeton, &format!("/api/members/{user_id}"))?;
            let u = m.get("member").unwrap_or(&m);
            match (u["customAvatar"].as_str(), u["avatar"].as_str()) {
                (Some(c), _) if c.starts_with('/') => format!("{ORIGINE}{c}"),
                (_, Some(a)) => a.to_string(),
                _ => return None,
            }
        }
    };
    let u = reqwest::Url::parse(&url).ok()?;
    // Seulement le portail (avec le jeton) ou le CDN de Discord.
    let req = match u.host_str() {
        Some(h) if h == crate::portail::hote() => client_direct().get(u).bearer_auth(jeton),
        Some("cdn.discordapp.com") => client_direct().get(u),
        _ => return None,
    };
    let octets = req.send().ok()?.bytes().ok()?;
    if octets.len() > 5 * 1024 * 1024 {
        return None;
    }
    let img = image::load_from_memory(&octets).ok()?.thumbnail(96, 96);
    std::fs::create_dir_all(cache).ok()?;
    img.save_with_format(&fichier, image::ImageFormat::Png).ok()?;
    Some(fichier)
}

fn appliquer_defcon(n: &Notifications, v: &Value, notifier: bool) {
    let actif = v["active"] == true;
    let niveau = if actif { v["level"].as_u64().unwrap_or(5).clamp(1, 5) as u8 } else { 5 };
    let avant = n.defcon.lock().unwrap().niveau;
    let reglages = n.reglages.lock().unwrap().clone();
    let mut d = n.defcon.lock().unwrap();
    let change = avant != niveau;
    d.niveau = niveau;
    d.nom = v["name"].as_str().map(str::to_string);
    d.message = v["message"].as_str().filter(|m| !m.is_empty()).map(str::to_string);
    if change {
        d.masque = false;
    }
    if notifier && change && reglages.defcon && avant != 0 {
        let titre = if niveau == 5 && !actif { "DEFCON levé — retour à la normale".to_string() } else { format!("DEFCON {niveau} — {}", d.nom.clone().unwrap_or_default()) };
        toast(&titre, d.message.as_deref().unwrap_or("Consigne sur le portail CES."), None);
    }
    d.bandeau = reglages.bandeau && niveau <= 3 && !d.masque;
}

// --- Boucle du direct --------------------------------------------------------------------------------------------

pub fn demarrer(app: &tauri::AppHandle, dossiers: &Dossiers) -> Notifications {
    let fichier = dossiers.donnees.join("notifications.json");
    let n = Notifications {
        reglages: Arc::new(Mutex::new(lire_reglages(&fichier))),
        vues: Arc::new(Mutex::new(std::collections::HashSet::new())),
        defcon: Arc::new(Mutex::new(EtatDefcon::default())),
        fichier,
    };
    let (a, cache) = (app.clone(), dossiers.donnees.join("avatars"));
    std::thread::Builder::new().name("direct".into()).spawn(move || boucle(a, cache)).expect("tâche du direct");
    let a2 = app.clone();
    std::thread::Builder::new().name("notifications".into()).spawn(move || boucle_notifications(a2)).expect("tâche des notifications");
    n
}

/// Notifications du portail → notifications Windows. Source principale : l'événement `notification:new` du direct
/// (instantané, audience = le destinataire). Filet : relecture de `/api/notifications` toutes les 3 minutes (direct
/// coupé, « reset » du bus). Au démarrage (ou à la connexion), les notifications existantes sont seulement mémorisées :
/// seules les nouvelles non lues s'affichent, 3 au plus par passage (puis un résumé).
fn boucle_notifications(app: tauri::AppHandle) {
    let mut jeton_vu: Option<String> = None;
    loop {
        std::thread::sleep(Duration::from_secs(if jeton_vu.is_some() { 180 } else { 20 }));
        let (Some(portail), Some(notifs)) = (app.try_state::<Portail>(), app.try_state::<Notifications>()) else { continue };
        let Ok(jeton) = jeton_actuel(&portail) else {
            jeton_vu = None;
            if let Some(n) = app.try_state::<Notifications>() {
                n.vues.lock().unwrap().clear();
            }
            continue;
        };
        let Some(v) = get(&jeton, "/api/notifications") else { continue };
        let liste: Vec<&Value> = v["items"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
        let premier = jeton_vu.as_deref() != Some(jeton.as_str());
        jeton_vu = Some(jeton.clone());
        let nouvelles: Vec<&Value> = liste
            .iter()
            .copied()
            .filter(|n| {
                let id = id_texte(&n["id"]);
                !id.is_empty() && notifs.vues.lock().unwrap().insert(id) && n["read"] != true
            })
            .collect();
        if premier || !notifs.reglages.lock().unwrap().portail {
            continue; // première lecture : on mémorise seulement ; sourdine : rien n'est affiché
        }
        for n in nouvelles.iter().take(3) {
            toast(n["title"].as_str().unwrap_or("Portail CES"), n["body"].as_str().unwrap_or("Nouvelle notification sur le portail CES."), None);
        }
        if nouvelles.len() > 3 {
            toast("Portail CES", &format!("{} autres notifications", nouvelles.len() - 3), None);
        }
    }
}

fn boucle(app: tauri::AppHandle, cache: PathBuf) {
    let mut curseur: Option<u64> = None;
    let mut jeton_vu: Option<String> = None;
    loop {
        let Some(portail) = app.try_state::<Portail>() else { std::thread::sleep(Duration::from_secs(2)); continue };
        let Some(notifs) = app.try_state::<Notifications>() else { std::thread::sleep(Duration::from_secs(2)); continue };
        let Ok(jeton) = jeton_actuel(&portail) else {
            curseur = None;
            jeton_vu = None;
            *notifs.defcon.lock().unwrap() = EtatDefcon::default();
            std::thread::sleep(Duration::from_secs(10));
            continue;
        };
        if jeton_vu.as_deref() != Some(jeton.as_str()) {
            // Connexion (ou reconnexion) : état DEFCON actuel, sans notification.
            if let Some(v) = get(&jeton, "/api/site/defcon") {
                appliquer_defcon(&notifs, &v, false);
            }
            jeton_vu = Some(jeton.clone());
        }
        let chemin = match curseur {
            Some(c) => format!("/api/chat/poll?since={c}"),
            None => "/api/chat/poll".to_string(),
        };
        let Some(v) = get(&jeton, &chemin) else {
            std::thread::sleep(Duration::from_secs(15));
            continue;
        };
        let seq = v["seq"].as_u64();
        if curseur.is_some() && v["reset"] != true {
            let moi = portail_moi(&portail);
            for ev in v["events"].as_array().into_iter().flatten() {
                let p = &ev["payload"];
                match ev["type"].as_str() {
                    Some("companion:launch") if notifs.reglages.lock().unwrap().lancements => {
                        let id = p["userId"].as_str().unwrap_or_default();
                        if Some(id) == moi.as_deref() {
                            continue;
                        }
                        let nom = p["username"].as_str().unwrap_or("Un membre");
                        let img = avatar(&jeton, &cache, id, p["avatarUrl"].as_str());
                        if notifs.reglages.lock().unwrap().carte {
                            crate::annonce::montrer(&app, nom, "a lancé Star Citizen", "lancement", img.as_deref());
                        } else {
                            toast(&format!("{nom} a lancé Star Citizen"), "CES Compagnon", img.as_deref());
                        }
                    }
                    Some("defcon") => appliquer_defcon(&notifs, p, true),
                    // Messagerie native : relais à l'interface (le portail ne publie à ce compte que ce qu'il peut lire).
                    Some(t @ ("message:new" | "message:update" | "message:delete" | "channels:changed")) => {
                        let _ = app.emit_to(EventTarget::webview("main"), "chat-evenement", serde_json::json!({ "type": t, "payload": p }));
                    }
                    // SOS : l'interface relit GET /api/sos (la notification Windows vient de notification:new).
                    Some("sos:update") => {
                        let _ = app.emit_to(EventTarget::webview("main"), "sos-evenement", p.clone());
                    }
                    Some("notification:new") => {
                        let id = id_texte(&p["id"]);
                        let nouvelle = !id.is_empty() && notifs.vues.lock().unwrap().insert(id);
                        let r = notifs.reglages.lock().unwrap().clone();
                        if nouvelle && p["type"] == "sos" && r.carte {
                            crate::annonce::montrer(&app, p["title"].as_str().unwrap_or("SOS"), p["body"].as_str().unwrap_or("Un membre demande de l'aide."), "sos", None);
                        } else if nouvelle && r.portail {
                            toast(p["title"].as_str().unwrap_or("Portail CES"), p["body"].as_str().unwrap_or("Nouvelle notification sur le portail CES."), None);
                        }
                    }
                    _ => {}
                }
            }
        }
        curseur = seq.or(curseur);
    }
}

fn portail_moi(portail: &Portail) -> Option<String> {
    crate::presence::utilisateur_id(portail)
}

// --- Commandes ------------------------------------------------------------------------------------------------

#[tauri::command]
pub fn notif_reglages(notifs: tauri::State<'_, Notifications>) -> Reglages {
    notifs.reglages.lock().unwrap().clone()
}

#[tauri::command]
pub fn notif_reglages_ecrire(app: tauri::AppHandle, notifs: tauri::State<'_, Notifications>, reglages: Reglages) -> Result<(), String> {
    if reglages.raccourcis_sos != notifs.reglages.lock().unwrap().raccourcis_sos {
        crate::raccourcis::appliquer(&app, reglages.raccourcis_sos)?;
    }
    crate::traduction::ecrire_sur(&notifs.fichier, serde_json::to_string(&reglages).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| e.to_string())?;
    *notifs.reglages.lock().unwrap() = reglages.clone();
    let mut d = notifs.defcon.lock().unwrap();
    d.bandeau = reglages.bandeau && d.niveau != 0 && d.niveau <= 3 && !d.masque;
    Ok(())
}

#[tauri::command]
pub fn notif_defcon(notifs: tauri::State<'_, Notifications>) -> EtatDefcon {
    notifs.defcon.lock().unwrap().clone()
}

/// Masque le bandeau jusqu'au prochain changement de niveau.
#[tauri::command]
pub fn bandeau_masquer(notifs: tauri::State<'_, Notifications>) {
    let mut d = notifs.defcon.lock().unwrap();
    d.masque = true;
    d.bandeau = false;
}

/// Notification d'essai (bouton « Tester » des réglages).
#[tauri::command]
pub fn notif_tester(app: tauri::AppHandle, notifs: tauri::State<'_, Notifications>) {
    if notifs.reglages.lock().unwrap().carte {
        crate::annonce::montrer(&app, "CES Compagnon", "Essai : les lancements de membres apparaîtront ainsi", "lancement", None);
    }
    toast("Notifications de CES Compagnon", "Elles apparaîtront ici : lancements de membres et alertes DEFCON.", None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echappement() {
        assert_eq!(echapper("a<b & \"c\">"), "a&lt;b &amp; &quot;c&quot;&gt;");
    }
}
