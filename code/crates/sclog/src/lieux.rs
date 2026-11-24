//! Noms de lieux internes du journal → noms lisibles (carnet de voyage).
//!
//! Le journal donne des identifiants techniques (`OOC_Stanton_4b_Clio`, `rs_ext_stan-pyro_jp1`,
//! `ab_mine_stanton4_med_012`, `MISSION_QT_Quantum_Beacon_…`). Cette table est tenue à la main et complétée au fil
//! des patchs ; un nom inconnu reste lisible (soulignés → espaces, suffixe numérique retiré).

const PLANETES: &[(&str, &str)] = &[
    ("stanton1", "Hurston"),
    ("stanton2", "Crusader"),
    ("stanton3", "ArcCorp"),
    ("stanton4", "microTech"),
];

const SYSTEMES: &[(&str, &str)] = &[
    ("stan", "Stanton"),
    ("stanton", "Stanton"),
    ("pyro", "Pyro"),
    ("nyx", "Nyx"),
    ("castra", "Castra"),
    ("magnus", "Magnus"),
    ("terra", "Terra"),
];

const CONNUS: &[(&str, &str)] = &[
    ("newbabbage", "New Babbage"),
    ("new_babbage", "New Babbage"),
    ("lorville", "Lorville"),
    ("area18", "Area 18"),
    ("orison", "Orison"),
    ("levski", "Levski"),
    ("grimhex", "Grim HEX"),
    ("ruin_station", "Ruin Station"),
    ("checkmate", "Checkmate"),
    ("gaslight", "Gaslight"),
    ("keeger", "Ceinture Keeger"),
    ("covalex", "Plateforme Covalex"),
    ("reststop", "Station de repos (R&R)"),
];

fn systeme(abrege: &str) -> String {
    let a = abrege.to_lowercase();
    SYSTEMES.iter().find(|(k, _)| *k == a).map(|(_, v)| v.to_string()).unwrap_or_else(|| majuscule(&a))
}

fn majuscule(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|p| p.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Nom par défaut : soulignés → espaces, suffixe numérique et préfixes techniques retirés.
fn generique(brut: &str) -> String {
    let mut parties: Vec<&str> = brut.split(['_', '-']).filter(|p| !p.is_empty()).collect();
    while parties.last().is_some_and(|p| p.chars().all(|c| c.is_ascii_digit())) {
        parties.pop();
    }
    let parties: Vec<&str> =
        parties.into_iter().filter(|p| !matches!(p.to_lowercase().as_str(), "loc" | "rs" | "ext" | "ooc" | "all")).collect();
    if parties.is_empty() {
        return brut.to_string();
    }
    // Noms collés en majuscules : « CovalexShippingHub » → « Covalex Shipping Hub »
    let mut sortie = String::new();
    for (i, p) in parties.iter().enumerate() {
        if i > 0 {
            sortie.push(' ');
        }
        let mut avant: Option<char> = None;
        for c in p.chars() {
            if c.is_uppercase() && avant.is_some_and(|a| a.is_lowercase()) {
                sortie.push(' ');
            }
            sortie.push(c);
            avant = Some(c);
        }
    }
    sortie
}

/// Traduit un nom de lieu interne en nom lisible (français).
pub fn lisible(brut: &str) -> String {
    let b = brut.trim();
    let bas = b.to_lowercase();
    let bas = bas.strip_prefix("loc_").unwrap_or(&bas).to_string();

    // Corps célestes : OOC_Stanton_4b_Clio → Clio ; OOC_Stanton_3_ArcCorp → ArcCorp
    if bas.starts_with("ooc_") {
        if let Some(nom) = b.rsplit('_').next().filter(|n| n.chars().any(|c| c.is_alphabetic())) {
            return nom.to_string();
        }
    }
    // Points de saut : rs_ext_stan-pyro_jp1 → Point de saut Stanton – Pyro
    if bas.contains("_jp") || bas.ends_with("jp1") {
        if let Some(seg) = bas.split('_').find(|s| s.contains('-')) {
            let (a, z) = seg.split_once('-').unwrap_or((seg, ""));
            return format!("Point de saut {} – {}", systeme(a), systeme(z));
        }
    }
    // Ceintures d'astéroïdes minières : ab_mine_stanton4_med_012 → Mine d'astéroïdes (microTech)
    if bas.starts_with("ab_mine") {
        let planete = PLANETES.iter().find(|(k, _)| bas.contains(k)).map(|(_, v)| *v);
        return match planete {
            Some(p) => format!("Mine d'astéroïdes ({p})"),
            None => "Mine d'astéroïdes".into(),
        };
    }
    // Points de Lagrange : rs_ext_pyro5_l2 → Pyro V – L2
    if let Some(l) = bas.split('_').find(|s| s.len() == 2 && s.starts_with('l') && s[1..].chars().all(|c| c.is_ascii_digit())) {
        if let Some(corps) = bas.split('_').find(|s| s.chars().next().is_some_and(|c| c.is_alphabetic()) && s.chars().last().is_some_and(|c| c.is_ascii_digit()) && *s != l) {
            let (nom, num): (String, String) = corps.chars().partition(|c| c.is_alphabetic());
            let romain = ["", "I", "II", "III", "IV", "V", "VI", "VII", "VIII"].get(num.parse::<usize>().unwrap_or(0)).copied().unwrap_or("");
            return format!("{} {} – {}", systeme(&nom), romain, l.to_uppercase()).replace("  ", " ");
        }
    }
    if bas.contains("quantum_beacon") || bas.starts_with("mission_qt") {
        return "Balise de mission".into();
    }
    if bas.starts_with("navpoint") {
        return "Point de navigation".into();
    }
    if bas.starts_with("partymembermarker") {
        return "Membre du groupe".into();
    }
    if let Some((_, v)) = CONNUS.iter().find(|(k, _)| bas.replace('_', "").contains(&k.replace('_', ""))) {
        return v.to_string();
    }
    generique(b)
}

/// Genre d'un lieu à partir de son nom LISIBLE (pour `location.enter`).
pub fn genre(lisible: &str) -> &'static str {
    let l = lisible.to_lowercase();
    if l.starts_with("point de saut") {
        "jumppoint"
    } else if l.starts_with("mine d'astéroïdes") || l.contains("ceinture") {
        "asteroid"
    } else if l.starts_with("balise") || l.starts_with("point de navigation") || l.starts_with("membre du groupe") {
        "waypoint"
    } else if l.contains(" – l") {
        "station"
    } else if ["new babbage", "lorville", "area 18", "orison", "levski"].iter().any(|v| l == *v) {
        "city"
    } else if ["station", "plateforme", "grim hex", "ruin", "checkmate", "gaslight", "gateway"].iter().any(|k| l.contains(k)) {
        "station"
    } else if ["hurston", "crusader", "arccorp", "microtech", "pyro i", "pyro ii", "pyro iii", "pyro iv", "pyro v", "pyro vi"].iter().any(|v| l == *v) {
        "planet"
    } else if ["arial", "aberdeen", "magda", "ita", "cellin", "daymar", "yela", "lyria", "wala", "calliope", "clio", "euterpe"].iter().any(|v| l == *v) {
        "moon"
    } else {
        "other"
    }
}

/// Nom de juridiction lisible tel que le HUD l'écrit (déjà lisible) ; « Aucune juridiction » conservé.
pub fn juridiction(texte: &str) -> String {
    texte.trim().trim_end_matches(':').trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lieux_du_journal_de_luka() {
        assert_eq!(lisible("OOC_Stanton_4b_Clio"), "Clio");
        assert_eq!(lisible("OOC_Stanton_3_ArcCorp"), "ArcCorp");
        assert_eq!(lisible("rs_ext_stan-pyro_jp1"), "Point de saut Stanton – Pyro");
        assert_eq!(lisible("LOC_rs_ext_stan-magnus_jp1"), "Point de saut Stanton – Magnus");
        assert_eq!(lisible("rs_ext_nyx-castra_jp1"), "Point de saut Nyx – Castra");
        assert_eq!(lisible("ab_mine_stanton4_med_012"), "Mine d'astéroïdes (microTech)");
        assert_eq!(lisible("MISSION_QT_Quantum_Beacon_LongRange_9996220573695"), "Balise de mission");
        assert_eq!(lisible("NavPoint_Dynamic_782672799521"), "Point de navigation");
        assert_eq!(lisible("PartyMemberMarker_781801744823"), "Membre du groupe");
        assert_eq!(lisible("NewBabbage_LOC"), "New Babbage");
        assert_eq!(lisible("levski_all-001"), "Levski");
        assert_eq!(lisible("rs_asmbl_keeger_04"), "Ceinture Keeger");
        assert_eq!(lisible("rs_ext_pyro5_l2"), "Pyro V – L2");
        assert_eq!(lisible("truc_inconnu_42"), "truc inconnu");
        assert_eq!(lisible("CovalexShippingHub"), "Plateforme Covalex");
        assert_eq!(lisible("ObjectContainer_RestStop_02"), "Station de repos (R&R)");
        assert_eq!(lisible("SomeNewPlace_7"), "Some New Place");
        assert_eq!(genre("Clio"), "moon");
        assert_eq!(genre("New Babbage"), "city");
        assert_eq!(genre("Point de saut Stanton – Pyro"), "jumppoint");
        assert_eq!(genre("Pyro V – L2"), "station");
        assert_eq!(genre("Balise de mission"), "waypoint");
    }
}
