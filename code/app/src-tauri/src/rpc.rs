//! Discord Rich Presence (demande du CES, 0.14.0) : « Dans le Verse » sur le profil Discord du membre pendant qu'il
//! joue à Star Citizen.
//!
//! 🔴 Passe par le canal LOCAL du client Discord (tube nommé `discord-ipc-N`), jamais par le jeu : rien n'est lu dans
//! sa mémoire ni injecté. Données affichées : état de présence déjà connu de l'application (menu / dans le Verse),
//! heure de début de session, lien vers le site public du CES. Désactivable (réglage `discord`).
//! Le nom affiché sur Discord (« Joue à … ») est celui de l'application Discord.

use crate::jeu::{iso_vers_secondes, EtatJeu};
use crate::notif::Notifications;
use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use sclog::Presence;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::Manager;

/// Identifiant PUBLIC de l'application Discord « CES Compagnon » (Discord Developer Portal, créée par le CES le 18/11).
/// `CES_DISCORD_APP_ID` à la construction le remplace (essais avec une autre application).
pub const APP_ID: Option<&str> = match option_env!("CES_DISCORD_APP_ID") {
    Some(i) => Some(i),
    None => Some("1572698406902702173"),
};

/// La Rich Presence est-elle disponible dans cette version (identifiant d'application fourni à la construction) ?
#[tauri::command]
pub fn discord_disponible() -> bool {
    APP_ID.is_some_and(|i| !i.is_empty())
}

pub fn demarrer(app: &tauri::AppHandle, jeu: Arc<Mutex<EtatJeu>>) {
    let Some(id) = APP_ID.filter(|i| !i.is_empty()) else { return };
    let (app, id) = (app.clone(), id.to_string());
    let _ = std::thread::Builder::new().name("discord".into()).spawn(move || boucle(app, jeu, id));
}

fn texte(etat: &EtatJeu) -> Option<(&'static str, String)> {
    let region = etat.region.as_deref().map(|r| match r {
        "eu" => "serveurs Europe".to_string(),
        "use" | "usw" | "us" => "serveurs Amérique".to_string(),
        "apse" | "asia" => "serveurs Asie".to_string(),
        autre => format!("serveurs {autre}"),
    });
    match etat.presence {
        Presence::Universe => Some(("Dans le Verse", region.map_or("Avec le CES".into(), |r| format!("Avec le CES · {r}")))),
        Presence::Menu => Some(("Au menu principal", "Avec le CES".into())),
        Presence::Launching => Some(("Lancement du jeu", "Avec le CES".into())),
        Presence::Offline => None,
    }
}

fn boucle(app: tauri::AppHandle, jeu: Arc<Mutex<EtatJeu>>, id: String) {
    let mut client: Option<DiscordIpcClient> = None;
    let mut affiche: Option<(String, String, Option<i64>)> = None;
    let mut tours = 0u32;
    loop {
        std::thread::sleep(Duration::from_secs(15));
        tours += 1;
        let actif = app.try_state::<Notifications>().is_some_and(|n| n.discord_actif());
        let etat = jeu.lock().unwrap().clone();
        let voulu = if actif { texte(&etat).map(|(d, s)| (d.to_string(), s, etat.session_debut.as_deref().and_then(iso_vers_secondes))) } else { None };
        // Demande de le CES (22/11) : passer devant les autres applications (Medal…). Discord n'offre aucune priorité
        // réglable ; on renvoie donc la même activité toutes les 60 s, ce qui la garde la plus récente.
        if voulu == affiche && (voulu.is_none() || tours % 4 != 0) {
            continue;
        }
        match &voulu {
            None => {
                if let Some(mut c) = client.take() {
                    let _ = c.clear_activity();
                    let _ = c.close();
                }
                affiche = None;
            }
            Some((details, state, debut)) => {
                if client.is_none() {
                    // Discord fermé : on réessaiera au prochain tour.
                    let Ok(mut c) = DiscordIpcClient::new(&id) else { continue };
                    if c.connect().is_err() {
                        continue;
                    }
                    client = Some(c);
                }
                let mut a = activity::Activity::new()
                    .details(details)
                    .state(state)
                    .assets(activity::Assets::new().large_image("https://app.ces-sc.fr/apple-touch-icon.png").large_text("CES — Corps Expéditionnaire Spatial"))
                    // Deux boutons au plus (limite de Discord) : le site public et la page pour rejoindre le CES.
                    .buttons(vec![
                        activity::Button::new("Visiter ces-sc.fr", "https://ces-sc.fr"),
                        activity::Button::new("Rejoindre le CES", "https://ces-sc.fr/rejoindre/"),
                    ]);
                if let Some(t) = debut {
                    a = a.timestamps(activity::Timestamps::new().start(*t));
                }
                let ok = client.as_mut().is_some_and(|c| c.set_activity(a).is_ok());
                if ok {
                    affiche = voulu.clone();
                } else {
                    client = None; // connexion perdue (Discord relancé) : nouvelle connexion au prochain tour
                }
            }
        }
    }
}
