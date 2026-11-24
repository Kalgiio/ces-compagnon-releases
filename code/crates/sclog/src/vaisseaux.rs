//! Noms de classes de vaisseaux du journal → noms lisibles (`KRIG_P52_Merlin` → « Kruger P52 Merlin »).
//!
//! Codes fabricants alignés sur ceux du portail. Les marqueurs techniques (PU, AI, CRIM…) et le suffixe numérique
//! final sont retirés ; les noms collés sont découpés (`AlphaWolf` → « Alpha Wolf »).

const FABRICANTS: &[(&str, &str)] = &[
    ("AEGS", "Aegis"),
    ("ANVL", "Anvil"),
    ("ARGO", "Argo"),
    ("BANU", "Banu"),
    ("CNOU", "Consolidated Outland"),
    ("CRUS", "Crusader"),
    ("DRAK", "Drake"),
    ("ESPR", "Esperia"),
    ("GAMA", "Gatac"),
    ("GLSN", "Gallenson"),
    ("GRIN", "Greycat"),
    ("KRIG", "Kruger"),
    ("MISC", "MISC"),
    ("MRAI", "Mirai"),
    ("ORIG", "Origin"),
    ("RSI", "RSI"),
    ("TMBL", "Tumbril"),
    ("VNCL", "Vanduul"),
    ("XIAN", "Aopoa"),
    ("XNAA", "Aopoa"),
];

/// Marqueurs techniques retirés du nom (variantes d'IA, de faction, gabarits).
const BRUIT: &[&str] = &["pu", "ai", "crim", "van", "vandull", "super", "npc", "unmanned", "template", "civ", "uee", "pir"];

/// Noms propres qui ne se découpent pas proprement.
const SPECIAUX: &[(&str, &str)] = &[("santokyai", "San'tok.yāi"), ("m80", "M80"), ("m50", "M50")];

fn decouper(mot: &str) -> String {
    if let Some((_, v)) = SPECIAUX.iter().find(|(k, _)| k.eq_ignore_ascii_case(mot)) {
        return v.to_string();
    }
    // Sigles et codes (F8C, P52, MOTH, ES) : tels quels
    if !mot.chars().any(|c| c.is_lowercase()) {
        return mot.to_string();
    }
    let mut s = String::new();
    let mut avant: Option<char> = None;
    for c in mot.chars() {
        if c.is_uppercase() && avant.is_some_and(|a| a.is_lowercase()) {
            s.push(' ');
        }
        s.push(c);
        avant = Some(c);
    }
    let mut c = s.chars();
    c.next().map(|p| p.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Nom lisible d'une classe de vaisseau. Une classe sans code fabricant connu est seulement mise en forme.
pub fn nom_vaisseau(classe: &str) -> String {
    let mut parties: Vec<&str> = classe.split('_').filter(|p| !p.is_empty()).collect();
    while parties.last().is_some_and(|p| p.chars().all(|c| c.is_ascii_digit())) {
        parties.pop();
    }
    if parties.is_empty() {
        return classe.to_string();
    }
    let fabricant = FABRICANTS.iter().find(|(k, _)| k.eq_ignore_ascii_case(parties[0])).map(|(_, v)| *v);
    let reste = if fabricant.is_some() { &parties[1..] } else { &parties[..] };
    let mots: Vec<String> = reste
        .iter()
        .filter(|p| !BRUIT.contains(&p.to_lowercase().as_str()))
        .map(|p| decouper(p))
        .collect();
    let corps = mots.join(" ");
    match (fabricant, corps.is_empty()) {
        (Some(f), false) => format!("{f} {corps}"),
        (Some(f), true) => f.to_string(),
        (None, _) if !corps.is_empty() => corps,
        _ => classe.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_des_journaux_de_luka() {
        assert_eq!(nom_vaisseau("KRIG_P52_Merlin"), "Kruger P52 Merlin");
        assert_eq!(nom_vaisseau("KRIG_L22_AlphaWolf"), "Kruger L22 Alpha Wolf");
        assert_eq!(nom_vaisseau("DRAK_Ironclad_Assault"), "Drake Ironclad Assault");
        assert_eq!(nom_vaisseau("ANVL_Lightning_F8C"), "Anvil Lightning F8C");
        assert_eq!(nom_vaisseau("RSI_Constellation_Aquila_GS"), "RSI Constellation Aquila GS");
        assert_eq!(nom_vaisseau("VNCL_Stinger_AI_Super_Vandull"), "Vanduul Stinger");
        assert_eq!(nom_vaisseau("VNCL_Stinger_PU_AI_VAN"), "Vanduul Stinger");
        assert_eq!(nom_vaisseau("RSI_Polaris_PU_AI_CRIM"), "RSI Polaris");
        assert_eq!(nom_vaisseau("XNAA_SanTokYai"), "Aopoa San'tok.yāi");
        assert_eq!(nom_vaisseau("ORIG_m80"), "Origin M80");
        assert_eq!(nom_vaisseau("MISC_Hull_B"), "MISC Hull B");
        assert_eq!(nom_vaisseau("GAMA_Railen"), "Gatac Railen");
        assert_eq!(nom_vaisseau("GLSN_Shiv"), "Gallenson Shiv");
        assert_eq!(nom_vaisseau("DRAK_Cutter_Rambler_930058506774"), "Drake Cutter Rambler");
        assert_eq!(nom_vaisseau("ARGO_MOTH"), "Argo MOTH");
        assert_eq!(nom_vaisseau("Truc_Inconnu"), "Truc Inconnu");
    }
}
