//! Découpage d'une ligne du journal.

use regex::Regex;
use std::sync::OnceLock;

/// Une ligne découpée. `etiquette` est le texte entre `<…>` qui suit le niveau (ex. `Join PU`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ligne<'a> {
    pub horodatage: Option<&'a str>,
    pub niveau: Option<&'a str>,
    pub etiquette: Option<&'a str>,
    pub texte: &'a str,
}

fn motif() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    M.get_or_init(|| {
        Regex::new(r"^<(\d{4}-\d\d-\d\dT[^>]+)>\s?(?:\[([A-Za-z]+)\]\s*)?(?:<([^<>]{1,90})>)?\s?(.*)$").unwrap()
    })
}

impl<'a> Ligne<'a> {
    pub fn decouper(brute: &'a str) -> Ligne<'a> {
        match motif().captures(brute) {
            Some(c) => Ligne {
                horodatage: c.get(1).map(|m| m.as_str()),
                niveau: c.get(2).map(|m| m.as_str()),
                etiquette: c.get(3).map(|m| m.as_str().trim()),
                texte: c.get(4).map(|m| m.as_str()).unwrap_or(""),
            },
            // suite d'un bloc multi-lignes (sans horodatage)
            None => Ligne { horodatage: None, niveau: None, etiquette: None, texte: brute },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ligne_complete() {
        let l = Ligne::decouper("<2026-11-13T16:46:23.938Z> [Notice] <Join PU> shard[pub_euw1b_1_200] [Team_GameServices]");
        assert_eq!(l.horodatage, Some("2026-11-13T16:46:23.938Z"));
        assert_eq!(l.niveau, Some("Notice"));
        assert_eq!(l.etiquette, Some("Join PU"));
        assert!(l.texte.starts_with("shard["));
    }

    #[test]
    fn ligne_sans_etiquette_et_suite() {
        let l = Ligne::decouper("<2026-11-13T16:42:23.124Z> Log started on Fri Nov 13 16:42:23 2026");
        assert_eq!(l.etiquette, None);
        assert_eq!(l.texte, "Log started on Fri Nov 13 16:42:23 2026");
        let s = Ligne::decouper("   Class: [SCTransportCarriage]");
        assert_eq!(s.horodatage, None);
    }
}
