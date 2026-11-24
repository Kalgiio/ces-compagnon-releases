//! Étude pour la proposition `location.enter` : rejoue des journaux et liste les entrées de lieux candidates
//! (arrivée d'un saut quantique avec sa destination, changement de juridiction), dédoublonnées par lieu sur 10 min.
//! N'affiche aucun pseudo (les événements n'en contiennent pas).
use sclog::{Analyseur, Evenement, Suiveur};
use std::collections::BTreeMap;

fn secondes(iso: &str) -> i64 {
    let n = |a: usize, z: usize| iso.get(a..z).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
    ((n(8, 10) * 24 + n(11, 13)) * 60 + n(14, 16)) * 60 + n(17, 19) + n(5, 7) * 31 * 86400
}

fn main() {
    let mut total_brut = 0usize;
    let mut total_dedup = 0usize;
    let mut genres: BTreeMap<String, usize> = BTreeMap::new();
    let mut exemples: Vec<String> = Vec::new();
    for chemin in std::env::args().skip(1) {
        let (mut s, mut a) = (Suiveur::nouveau(&chemin), Analyseur::nouveau());
        let mut dernier: BTreeMap<String, i64> = BTreeMap::new();
        loop {
            let l = s.lire().expect("lecture");
            for (p, ligne) in &l.lignes {
                for e in a.traiter(*p, ligne) {
                    let (lieu, genre, at) = match &e {
                        Evenement::SautQuantique { destination: Some(d), at, .. } => (d.clone(), sclog::lieux::genre(d), at.clone()),
                        Evenement::Juridiction { nom, at, .. } => (nom.clone(), "jurisdiction", at.clone()),
                        _ => continue,
                    };
                    total_brut += 1;
                    let t = secondes(&at);
                    if dernier.get(&lieu).is_some_and(|x| t - x < 600) {
                        continue;
                    }
                    dernier.insert(lieu.clone(), t);
                    total_dedup += 1;
                    *genres.entry(genre.to_string()).or_default() += 1;
                    if exemples.len() < 25 {
                        exemples.push(format!("{at}  {{ \"location\": \"{lieu}\", \"kind\": \"{genre}\" }}"));
                    }
                }
            }
            if !l.suite {
                break;
            }
        }
    }
    println!("candidats bruts : {total_brut} · après dédoublonnage 10 min : {total_dedup}");
    println!("par genre : {genres:?}");
    for e in exemples {
        println!("  {e}");
    }
}
