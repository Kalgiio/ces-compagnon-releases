//! `sclog` — le `Game.log` de Star Citizen vu par CES Compagnon.
//!
//! - [`ligne`] : découpage d'une ligne `<horodatage> [Niveau] <Étiquette> texte`.
//! - [`suivi`] : lecture en direct de la fin du fichier (le jeu continue d'écrire), rotation détectée.
//! - [`analyse`] : état de présence et événements **du joueur local uniquement**.
//!
//! Règles du projet : lecture seule d'un fichier écrit par le jeu. Aucun pseudo ni identifiant d'un autre
//! joueur ne sort de ce module (contrat d'API § 1) : l'analyse ne les compare qu'à ceux du joueur local.

pub mod analyse;
pub mod lieux;
pub mod ligne;
pub mod suivi;
pub mod vaisseaux;

pub use analyse::{Analyseur, Evenement, Presence};
pub use suivi::{Lecture, Suiveur};
