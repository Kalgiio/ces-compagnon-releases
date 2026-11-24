//! Rejoue de vrais journaux et affiche, par fichier, le nombre d'événements de chaque type et l'état final.
//! N'affiche aucun pseudo ni identifiant (les événements n'en contiennent pas).
//!
//! cargo run --release --example rejouer -- "F:\RSI\StarCitizen\LIVE\Game.log" "F:\...\logbackups\*.log"

use sclog::{Analyseur, Evenement, Suiveur};
use std::collections::BTreeMap;

fn main() {
    for chemin in std::env::args().skip(1) {
        let mut s = Suiveur::nouveau(&chemin);
        let mut a = Analyseur::nouveau();
        let mut compte: BTreeMap<String, usize> = BTreeMap::new();
        let mut presences = Vec::new();
        loop {
            let l = s.lire().expect("lecture");
            for (pos, ligne) in &l.lignes {
                for e in a.traiter(*pos, ligne) {
                    let t = match &e {
                        Evenement::Presence { state, region, .. } => {
                            presences.push(format!("{:?}{}", state, region.as_ref().map(|r| format!("({r})")).unwrap_or_default()).to_lowercase());
                            "presence".to_string()
                        }
                        Evenement::VaisseauDetruit { role, reason, .. } => format!("vehicle.destroyed[{role},{reason}]"),
                        Evenement::JoueurATerre { kind, .. } => format!("player.down[{kind}]"),
                        other => serde_type(other),
                    };
                    *compte.entry(t).or_default() += 1;
                }
            }
            if !l.suite {
                break;
            }
        }
        let nom = std::path::Path::new(&chemin).file_name().unwrap().to_string_lossy().into_owned();
        println!("{nom}\n  build {:?} version {:?}\n  {:?}\n  présence : {}\n", a.build(), a.version(), compte, presences.join(" → "));
    }
}

fn serde_type(e: &Evenement) -> String {
    match e {
        Evenement::SessionDebut { .. } => "session.start",
        Evenement::SessionFin { .. } => "session.end",
        Evenement::SautQuantique { .. } => "travel.quantum",
        Evenement::MissionTerminee { .. } => "mission.completed",
        _ => "?",
    }
    .to_string()
}
