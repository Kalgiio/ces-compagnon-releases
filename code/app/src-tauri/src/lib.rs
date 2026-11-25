//! Cœur de CES Compagnon (squelette, étape 3.1).
//!
//! Règles d'or (PROMPT-PC.md § 2) : l'application reste EXTÉRIEURE au jeu. Elle lit la liste des
//! processus Windows et les fichiers que le jeu écrit ; jamais la mémoire du processus, jamais
//! d'injection, de hook ni d'interception réseau.

mod choix;
mod conditions;
mod canal;
mod carnet;
mod commerce;
mod compte;
mod file;
mod jeu;
mod minage;
mod ocr;
mod partage_touches;
mod portail;
mod site;
mod notif;
mod rpc;
mod lanceur;
mod sos;
mod natif;
mod fond;
mod annonce;
mod raccourcis;
mod vignette;
mod personnages;
mod presence;
mod touches;
mod traduction;
mod wikelo;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WindowEvent,
};

/// Montre et met au premier plan la fenêtre principale (depuis l'icône de notification).
fn montrer(app: &AppHandle) {
    if let Some(f) = app.get_webview_window("main") {
        let _ = f.unminimize();
        let _ = f.show();
        let _ = f.set_focus();
    }
}

/// Démarrage avec Windows : actif ?
#[tauri::command]
fn demarrage_etat(app: tauri::AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// Active ou coupe le démarrage avec Windows (entrée « Exécuter » de l'utilisateur, aucun droit administrateur).
#[tauri::command]
fn demarrage_regler(app: tauri::AppHandle, actif: bool) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    let a = app.autolaunch();
    if actif { a.enable() } else { a.disable() }.map_err(|e| e.to_string())?;
    a.is_enabled().map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Mises à jour signées (clé publique dans tauri.conf.json), canal « canary » du dépôt public
        // Kalgiio/ces-compagnon-releases.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // Démarrage avec Windows (demande du CES, 23/11) : réglage dans Outils, désactivé par défaut. Lancée ainsi,
        // l'application part rangée dans la zone de notification (argument --autostart).
        // SOS au clavier (raccourcis globaux, désactivés par défaut) : raccourcis.rs.
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(|app, s, e| raccourcis::sur_raccourci(app, s, e.state())).build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--autostart"])))
        .register_asynchronous_uri_scheme_protocol("cesmedia", site::protocole_media)
        .invoke_handler(tauri::generate_handler![
            jeu::etat,
            presence::compte_etat,
            presence::appairage_debut,
            presence::appairage_suivi,
            presence::ouvrir_page,
            presence::deconnexion,
            presence::reglages_lire,
            presence::reglages_ecrire,
            traduction::traduction_etat,
            traduction::traduction_installer,
            traduction::traduction_restaurer,
            traduction::traduction_sauvegardes,
            touches::touches_lire,
            touches::touches_sauvegardes,
            touches::touches_sauvegarder,
            touches::touches_restaurer,
            touches::touches_exporter,
            touches::touches_importer,
            partage_touches::touches_publier,
            partage_touches::touches_partages,
            partage_touches::touches_membres,
            partage_touches::touches_recuperer,
            partage_touches::touches_retirer_partage,
            commerce::commerce_donnees,
            commerce::commerce_routes,
            commerce::uex_jeton_etat,
            commerce::uex_jeton_enregistrer,
            commerce::uex_jeton_effacer,
            commerce::uex_ouvrir_page,
            minage::minage_referentiel,
            minage::minage_bilan,
            minage::minage_commencer,
            minage::minage_enregistrer,
            minage::minage_supprimer,
            minage::minage_exporter,
            wikelo::wikelo_liste,
            wikelo::wikelo_detail,
            conditions::conditions_etat,
            raccourcis::raccourci_verifier,
            raccourcis::raccourcis_suspendre,
            conditions::conditions_accepter,
            conditions::conditions_ouvrir_lien,
            wikelo::wikelo_choisir_image,
            wikelo::wikelo_proposer,
            sos::sos_action,
            natif::portail_api,
            personnages::personnages_locaux,
            personnages::personnages_portail,
            personnages::personnage_choisir_photo,
            personnages::personnage_envoyer,
            personnages::personnage_photo,
            personnages::personnage_recuperer,
            fond::fond_perso_lire,
            fond::fond_perso_choisir,
            fond::fond_perso_effacer,
            lanceur::rsi_lanceur_etat,
            lanceur::rsi_lanceur_ouvrir,
            lanceur::rsi_lanceur_choisir,
            wikelo::wikelo_suivi,
            wikelo::wikelo_favori,
            wikelo::wikelo_reunir,
            carnet::carnet_sessions,
            carnet::carnet_session,
            carnet::carnet_importer,
            carnet::carnet_lieux,
            ocr::ocr_raffinerie,
            canal::canal_etat,
            canal::canal_choisir,
            canal::maj_verifier,
            canal::maj_installer,
            jeu::jeu_choisir_dossier,
            jeu::jeu_dossier_auto,
            site::site_ouvrir,
            site::site_placer,
            site::site_masquer,
            site::site_navigateur,
            site::site_ouvrir_lien,
            site::portal_fetch,
            site::notifications_non_lues,
            site::site_notifications,
            site::site_page,
            site::notifications_action,
            site::site_afficher,
            site::statut_ces,
            site::statut_ouvrir,
            rpc::discord_disponible,
            notif::notif_reglages,
            notif::notif_reglages_ecrire,
            notif::notif_defcon,
            notif::bandeau_masquer,
            notif::notif_tester,
            demarrage_etat,
            vignette::vignette_radio,
            demarrage_regler
        ])
        .setup(|app| {
            // Surveillance du jeu (processus + journal) puis présence envoyée au portail.
            let dossier = app.path().app_data_dir().expect("dossier de données de l'application");
            let carnet = std::sync::Arc::new(carnet::Carnet::ouvrir(&dossier).expect("carnet de voyage"));
            let surveillance = jeu::demarrer(carnet.clone(), &dossier);
            app.manage(carnet);
            app.manage(presence::demarrer(surveillance.etat.clone(), surveillance.sortie.clone(), &dossier));
            app.manage(notif::demarrer(app.handle(), &traduction::Dossiers { donnees: dossier.clone() }));
            rpc::demarrer(app.handle(), surveillance.etat.clone());
            app.manage(surveillance);
            app.manage(traduction::Traduction::default());
            app.manage(wikelo::SuiviWikelo(std::sync::Mutex::new(None)));
            app.manage(traduction::Dossiers { donnees: dossier.clone() });
            app.manage(raccourcis::Attente::default());
            app.manage(raccourcis::Actifs::default());
            vignette::installer(app.handle());
            let (sos_actif, sos_touches) = app.state::<notif::Notifications>().raccourcis_sos();
            if sos_actif {
                if let Err(e) = raccourcis::appliquer(app.handle(), true, &sos_touches) {
                    eprintln!("[raccourcis] {e}");
                }
            }
            app.manage(minage::Minage::ouvrir(&traduction::Dossiers { donnees: dossier.clone() }).expect("base du minage"));

            if std::env::args().any(|a| a == "--autostart") {
                if let Some(f) = app.get_webview_window("main") {
                    let _ = f.hide();
                }
            }
            let ouvrir = MenuItem::with_id(app, "ouvrir", "Ouvrir CES Compagnon", true, None::<&str>)?;
            let quitter = MenuItem::with_id(app, "quitter", "Quitter", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&ouvrir, &quitter])?;
            TrayIconBuilder::with_id("principal")
                .icon(app.default_window_icon().cloned().expect("icône de l'application"))
                .tooltip("CES Compagnon")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "ouvrir" => montrer(app),
                    "quitter" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|icone, e| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                        montrer(icone.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        // Fermer la fenêtre la RANGE dans la zone de notification : la surveillance continue.
        // Pour quitter vraiment : menu de l'icône › Quitter.
        .on_window_event(|fenetre, e| {
            if let WindowEvent::CloseRequested { api, .. } = e {
                let _ = fenetre.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("erreur au démarrage de CES Compagnon");
}
