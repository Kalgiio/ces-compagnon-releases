//! Rejoue un journal et affiche les étapes du carnet de voyage (aucun pseudo : les événements n'en contiennent pas).
use sclog::{Analyseur, Evenement, Suiveur};
fn main() {
    for chemin in std::env::args().skip(1) {
        let (mut s, mut a) = (Suiveur::nouveau(&chemin), Analyseur::nouveau());
        let mut n = std::collections::BTreeMap::<&str, usize>::new();
        let mut dest = Vec::new();
        loop {
            let l = s.lire().expect("lecture");
            for (p, ligne) in &l.lignes {
                for e in a.traiter(*p, ligne) {
                    let k = match &e {
                        Evenement::RouteCalculee { destination, .. } => { dest.push(destination.clone()); "route" }
                        Evenement::Juridiction { .. } => "juridiction",
                        Evenement::Armistice { .. } => "armistice",
                        Evenement::SautQuantique { destination, .. } => { if destination.is_some() { "saut+dest" } else { "saut" } }
                        _ => continue,
                    };
                    *n.entry(k).or_default() += 1;
                }
            }
            if !l.suite { break; }
        }
        dest.sort(); dest.dedup();
        println!("{} : {:?}\n  destinations : {}", std::path::Path::new(&chemin).file_name().unwrap().to_string_lossy(), n, dest.join(" | "));
    }
}
