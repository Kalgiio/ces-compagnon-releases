//! sclog-probe — sonde du `Game.log` de Star Citizen (CES Compagnon, étape 1).
//!
//! Lit un ou plusieurs journaux EN FLUX (jamais chargés entiers), classe chaque ligne par type
//! d'événement, compte, garde 3 exemples par type, puis ANONYMISE les exemples au moment d'écrire
//! le rapport Markdown (les pseudos découverts plus loin dans le fichier sont donc aussi masqués
//! dans les exemples gardés plus tôt).
//!
//! Règles du projet : lecture seule des fichiers écrits par le jeu, rien d'autre (ni mémoire du
//! processus, ni réseau). Le rapport ne contient jamais de ligne brute non anonymisée.
//!
//! Usage : sclog-probe [--out rapport.md] [--redact MOT]... [--top N] [--analyse analyse.md] <journal.log>...
//! Diagnostic local : SCLOG_PSEUDOS=1 affiche la table des pseudos sur la console (jamais dans le rapport).

use regex::Regex;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

const EXEMPLES_PAR_TYPE: usize = 3;
const LONGUEUR_MAX_EXEMPLE: usize = 400;

/// Mots-clés qui signalent un type utile aux objectifs du compagnon (§ 4 de PROMPT-PC.md).
/// Les types qui en contiennent un ont leurs exemples détaillés dans le rapport.
const MOTS_INTERESSANTS: &[(&str, &str)] = &[
    ("death", "éliminations"),
    ("kill", "éliminations"),
    ("destr", "éliminations"),
    ("corpse", "éliminations"),
    ("incap", "éliminations"),
    ("respawn", "éliminations"),
    ("medical", "éliminations"),
    ("join pu", "présence"),
    ("login", "présence"),
    ("change server", "présence"),
    ("systemquit", "présence"),
    ("disconnect", "présence"),
    ("channel", "présence"),
    ("expect incoming", "présence"),
    ("quantum", "voyages"),
    ("travel", "voyages"),
    ("location", "voyages"),
    ("jump", "voyages"),
    ("zone", "voyages"),
    ("route", "voyages"),
    ("atc", "voyages"),
    ("landing", "voyages"),
    ("mission", "missions"),
    ("objective", "missions"),
    ("contract", "missions"),
    ("shop", "commerce"),
    ("commodit", "commerce"),
    ("cargo", "commerce"),
    ("rental", "commerce"),
    ("mining", "minage"),
    ("refin", "minage"),
    ("mineable", "minage"),
    ("party", "groupe"),
    ("vehicle", "véhicules"),
    ("spawn", "véhicules"),
    ("hangar", "véhicules"),
    ("inventory", "inventaire"),
    ("equip", "inventaire"),
    ("crime", "sécurité"),
    ("jurisdiction", "sécurité"),
    ("juridiction", "sécurité"),
    ("armistice", "sécurité"),
    ("notification", "notifications du HUD"),
    ("instanc", "voyages"),
    ("eac login", "présence"),
];

#[derive(Default)]
struct TypeStats {
    count: u64,
    exemples: Vec<String>,
    premier: Option<String>,
    dernier: Option<String>,
    fichiers: HashSet<usize>,
}

#[derive(Default)]
struct FichierInfo {
    nom: String,
    taille: u64,
    lignes: u64,
    debut: Option<String>,
    fin: Option<String>,
    build: Option<String>,
    version: Option<String>,
    branche: Option<String>,
    shards: BTreeMap<String, u64>,
}

struct Regles {
    ligne: Regex,
    cont_norm: Regex,
    noms: Vec<Regex>,
    nom_valide: Regex,
    build: Regex,
    version: Regex,
    branche: Regex,
    shard: Regex,
    // anonymisation
    guid: Regex,
    hex_long: Regex,
    hex_0x: Regex,
    ipv4: Regex,
    email: Regex,
    chemin_user: Regex,
    nombres_longs: Regex,
}

impl Regles {
    fn new() -> Self {
        Regles {
            // <horodatage> [Niveau] <Étiquette> reste
            ligne: Regex::new(r"^<(\d{4}-\d\d-\d\dT[^>]+)>\s?(?:\[([A-Za-z]+)\]\s*)?(?:<([^<>]{1,90})>)?\s?(.*)$").unwrap(),
            cont_norm: Regex::new(r"\d+").unwrap(),
            // Motifs FIABLES où le jeu écrit un pseudo (observés dans de vrais journaux, 4.x).
            // Les deux premiers donnent le joueur local. Un pseudo repéré une fois est ensuite
            // masqué PARTOUT dans le rapport, quelle que soit la ligne.
            noms: vec![
                Regex::new(r"Handle\[([^\]\s]{2,40})\]").unwrap(),
                Regex::new(r#"nickname="([^"]{2,40})""#).unwrap(),
                Regex::new(r" name ([A-Za-z0-9_\-]{2,40}) - state").unwrap(),
                Regex::new(r"(?:killer|victim|attacker|target) '([^']{2,40})'").unwrap(),
                Regex::new(r"with user \[([A-Za-z0-9_\-]{2,40})\]").unwrap(),
                Regex::new(r"Player ([A-Za-z0-9_\-]{2,40})\[\d+\]").unwrap(),
                Regex::new(r"\[([A-Za-z0-9_\-]{2,40})\] \[AI 0\]").unwrap(),
                Regex::new(r" for ([A-Za-z0-9_\-]{2,40}) \[\d+\] to track").unwrap(),
                Regex::new(r#"(?:^|")([A-Za-z0-9_\-]{2,40}) (?:s'est connecté|s'est déconnecté|a rejoint|a quitté|has joined|has left|joined|left)"#).unwrap(),
                Regex::new(r#"[Nn]otification "([A-Za-z0-9_\-]{2,40}) !"#).unwrap(),
                Regex::new(r"CANAL '[^':]{1,60} : ([A-Za-z0-9_\-]{2,40})'").unwrap(),
                Regex::new(r"(?:CHANNEL|Channel) '[^':]{1,60} : ([A-Za-z0-9_\-]{2,40})'").unwrap(),
                Regex::new(r"(?:démarrée|lancée|started|initiated) (?:par|by) ([A-Za-z0-9_\-]{2,40})\.").unwrap(),
                Regex::new(r"zone hosted child id = \d+ name = ([A-Za-z0-9_\-]{2,40}) ").unwrap(),
            ],
            // Un pseudo RSI : pas d'identifiant numérique long collé (sinon c'est une entité du jeu)
            nom_valide: Regex::new(r"^[A-Za-z0-9_\-]{2,40}$").unwrap(),
            build: Regex::new(r"Build\((\d+)\)").unwrap(),
            version: Regex::new(r"FileVersion: ([\d.]+)").unwrap(),
            branche: Regex::new(r"(?i)branch[:=\s]+\[?([A-Za-z0-9_.\-]+)").unwrap(),
            shard: Regex::new(r"shard\[([^\]]+)\]").unwrap(),
            guid: Regex::new(r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b").unwrap(),
            hex_long: Regex::new(r"(?i)\b[0-9a-f]{16,}\b").unwrap(),
            hex_0x: Regex::new(r"(?i)\b0x[0-9a-f]{6,}\b").unwrap(),
            ipv4: Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap(),
            email: Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").unwrap(),
            chemin_user: Regex::new(r"(?i)([A-Z]:[\\/]+Users[\\/]+)[^\\/\s]+").unwrap(),
            nombres_longs: Regex::new(r"-?\d{6,}").unwrap(),
        }
    }

    fn est_pseudo(&self, s: &str) -> bool {
        if !self.nom_valide.is_match(s) {
            return false;
        }
        // entités du jeu : DRAK_Cutter_930058506774, AImodule_…, PU_…
        if self.nombres_longs.is_match(s) {
            return false;
        }
        let bas = s.to_ascii_lowercase();
        !(bas.starts_with("aimodule") || bas.starts_with("pu_") || bas == "unknown" || bas == "none")
    }
}

struct Sonde {
    r: Regles,
    types: HashMap<String, TypeStats>,
    fichiers: Vec<FichierInfo>,
    pseudos: Vec<String>, // dans l'ordre de découverte
    pseudos_vus: HashSet<String>,
    pseudo_local: Option<String>,
}

impl Sonde {
    fn new() -> Self {
        Sonde {
            r: Regles::new(),
            types: HashMap::new(),
            fichiers: Vec::new(),
            pseudos: Vec::new(),
            pseudos_vus: HashSet::new(),
            pseudo_local: None,
        }
    }

    fn noter_pseudo(&mut self, p: &str) {
        if self.r.est_pseudo(p) && self.pseudos_vus.insert(p.to_string()) {
            self.pseudos.push(p.to_string());
        }
    }

    fn lire(&mut self, chemin: &PathBuf) -> std::io::Result<()> {
        let idx = self.fichiers.len();
        let f = File::open(chemin)?;
        let mut info = FichierInfo {
            nom: chemin.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
            taille: f.metadata()?.len(),
            ..Default::default()
        };
        let mut lecteur = BufReader::with_capacity(1 << 20, f);
        let mut tampon = Vec::with_capacity(4096);
        loop {
            tampon.clear();
            if lecteur.read_until(b'\n', &mut tampon)? == 0 {
                break;
            }
            let ligne = String::from_utf8_lossy(&tampon);
            let ligne = ligne.trim_end_matches(['\r', '\n']);
            if ligne.is_empty() {
                continue;
            }
            info.lignes += 1;
            self.traiter(ligne, idx, &mut info);
        }
        self.fichiers.push(info);
        Ok(())
    }

    fn traiter(&mut self, ligne: &str, idx: usize, info: &mut FichierInfo) {
        // Pseudos : à chercher partout, y compris hors des types gardés en exemple.
        let mut trouves: Vec<String> = Vec::new();
        for re in &self.r.noms {
            for c in re.captures_iter(ligne) {
                trouves.push(c[1].to_string());
            }
        }
        if self.pseudo_local.is_none() {
            if let Some(c) = self.r.noms[0].captures(ligne).or_else(|| self.r.noms[1].captures(ligne)) {
                if self.r.est_pseudo(&c[1]) {
                    self.pseudo_local = Some(c[1].to_string());
                }
            }
        }
        for p in trouves {
            self.noter_pseudo(&p);
        }

        let (cle, horodatage) = match self.r.ligne.captures(ligne) {
            Some(c) => {
                let ts = c.get(1).map(|m| m.as_str().to_string());
                let niveau = c.get(2).map(|m| m.as_str());
                let etiquette = c.get(3).map(|m| m.as_str().trim());
                let reste = c.get(4).map(|m| m.as_str()).unwrap_or("");
                let cle = match (niveau, etiquette) {
                    (_, Some(e)) => format!("[{}] <{}>", niveau.unwrap_or("-"), e),
                    (Some(n), None) => format!("[{}] (sans étiquette) {}", n, self.prefixe(reste)),
                    (None, None) => format!("(sans niveau) {}", self.prefixe(reste)),
                };
                (cle, ts)
            }
            None => (format!("(suite de ligne) {}", self.prefixe(ligne)), None),
        };

        if let Some(ts) = &horodatage {
            if info.debut.is_none() {
                info.debut = Some(ts.clone());
            }
            info.fin = Some(ts.clone());
        }
        if info.build.is_none() {
            if let Some(c) = self.r.build.captures(ligne) {
                info.build = Some(c[1].to_string());
            }
        }
        if info.version.is_none() {
            if let Some(c) = self.r.version.captures(ligne) {
                info.version = Some(c[1].to_string());
            }
        }
        if info.branche.is_none() && ligne.to_ascii_lowercase().contains("branch") {
            if let Some(c) = self.r.branche.captures(ligne) {
                info.branche = Some(c[1].to_string());
            }
        }
        if let Some(c) = self.r.shard.captures(ligne) {
            *info.shards.entry(c[1].to_string()).or_default() += 1;
        }

        let st = self.types.entry(cle).or_default();
        st.count += 1;
        st.fichiers.insert(idx);
        if let Some(ts) = horodatage {
            if st.premier.is_none() {
                st.premier = Some(ts.clone());
            }
            st.dernier = Some(ts);
        }
        if st.exemples.len() < EXEMPLES_PAR_TYPE {
            let ex: String = ligne.chars().take(LONGUEUR_MAX_EXEMPLE).collect();
            if !st.exemples.contains(&ex) {
                st.exemples.push(ex);
            }
        }
    }

    /// Clé de regroupement d'une ligne sans étiquette : 3 premiers mots, chiffres → N.
    fn prefixe(&self, s: &str) -> String {
        let mots: Vec<&str> = s.split_whitespace().take(3).collect();
        let p = mots.join(" ");
        let p = self.r.cont_norm.replace_all(&p, "N");
        p.chars().take(48).collect()
    }

    /// Anonymisation d'un texte destiné au rapport.
    fn anonymiser(&self, s: &str, a: &Anonymiseur, garder: &HashSet<String>) -> String {
        let r = &self.r;
        let mut t = r.email.replace_all(s, "<EMAIL>").into_owned();
        t = r.chemin_user.replace_all(&t, "${1}<UTILISATEUR>").into_owned();
        t = r.guid.replace_all(&t, "<GUID>").into_owned();
        t = r.hex_0x.replace_all(&t, "<ADR>").into_owned();
        t = r.hex_long.replace_all(&t, "<HEX>").into_owned();
        t = r.ipv4.replace_all(&t, "<IP>").into_owned();
        if let Some(re) = &a.noms {
            // jetons de mot : on remplace un mot entier égal (sans casse) à un pseudo connu
            t = re
                .replace_all(&t, |c: &regex::Captures| {
                    a.table.get(&c[0].to_lowercase()).cloned().unwrap_or_else(|| c[0].to_string())
                })
                .into_owned();
        }
        if let Some(re) = &a.redact {
            t = re.replace_all(&t, "<MASQUÉ>").into_owned();
        }
        t = r
            .nombres_longs
            .replace_all(&t, |c: &regex::Captures| {
                let n = c[0].trim_start_matches('-');
                if garder.contains(n) {
                    c[0].to_string()
                } else {
                    "<ID>".to_string()
                }
            })
            .into_owned();
        t
    }
}

/// Pseudos connus → jetons, compilés UNE fois en une seule expression.
struct Anonymiseur {
    table: HashMap<String, String>, // clé en minuscules
    noms: Option<Regex>,
    redact: Option<Regex>,
}

impl Anonymiseur {
    fn new(table: &HashMap<String, String>, redact: &[String]) -> Self {
        let mut noms: Vec<&String> = table.keys().collect();
        noms.sort_by(|a, b| b.len().cmp(&a.len()));
        // Un « mot » = suite de [A-Za-z0-9_-] ; le pseudo doit occuper le mot entier.
        let alt = noms.iter().map(|n| regex::escape(n)).collect::<Vec<_>>().join("|");
        let re_noms = if alt.is_empty() {
            None
        } else {
            Some(Regex::new(&format!(r"(?i)\b(?:{})\b", alt)).unwrap())
        };
        let red: Vec<String> = redact.iter().filter(|m| !m.is_empty()).map(|m| regex::escape(m)).collect();
        let re_red = if red.is_empty() { None } else { Some(Regex::new(&format!("(?i){}", red.join("|"))).unwrap()) };
        Anonymiseur {
            table: table.iter().map(|(k, v)| (k.to_lowercase(), v.clone())).collect(),
            noms: re_noms,
            redact: re_red,
        }
    }
}

fn categories(cle: &str) -> Vec<&'static str> {
    let bas = cle.to_lowercase();
    let mut v: Vec<&'static str> = Vec::new();
    for (mot, cat) in MOTS_INTERESSANTS {
        if bas.contains(mot) && !v.contains(cat) {
            v.push(cat);
        }
    }
    v
}

fn md(s: &str) -> String {
    s.replace('|', "\\|").replace('`', "'")
}

fn taille_lisible(o: u64) -> String {
    let mo = o as f64 / 1_048_576.0;
    if mo >= 1.0 {
        format!("{:.1} Mo", mo)
    } else {
        format!("{:.0} Ko", o as f64 / 1024.0)
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut sortie = PathBuf::from("log-report.md");
    let mut redact: Vec<String> = Vec::new();
    let mut fichiers: Vec<PathBuf> = Vec::new();
    let mut top_autres: usize = 40;
    let mut analyse: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--analyse" => analyse = Some(PathBuf::from(args.next().expect("--analyse <fichier.md>"))),
            "--out" => sortie = PathBuf::from(args.next().expect("--out <fichier>")),
            "--redact" => redact.push(args.next().expect("--redact <mot>")),
            "--top" => top_autres = args.next().expect("--top <n>").parse().expect("nombre"),
            "-h" | "--help" => {
                println!("Usage : sclog-probe [--out rapport.md] [--redact MOT]... [--top N] <journal.log>...");
                return;
            }
            _ => fichiers.push(PathBuf::from(a)),
        }
    }
    if fichiers.is_empty() {
        eprintln!("Aucun journal fourni. Usage : sclog-probe [--out rapport.md] <Game.log>...");
        std::process::exit(2);
    }

    let mut sonde = Sonde::new();
    for f in &fichiers {
        eprintln!("lecture : {}", f.display());
        if let Err(e) = sonde.lire(f) {
            eprintln!("  ⚠ illisible : {e}");
        }
    }

    // Table des pseudos : le joueur local d'abord.
    let mut table: HashMap<String, String> = HashMap::new();
    if let Some(l) = &sonde.pseudo_local {
        table.insert(l.clone(), "<JOUEUR_LOCAL>".to_string());
    }
    let mut n = 0;
    for p in &sonde.pseudos {
        if !table.contains_key(p) {
            n += 1;
            table.insert(p.clone(), format!("<JOUEUR_{n}>"));
        }
    }
    // Nombres longs à NE PAS masquer : numéros de build (utiles, non personnels).
    let garder: HashSet<String> = sonde.fichiers.iter().filter_map(|f| f.build.clone()).collect();
    if std::env::var_os("SCLOG_PSEUDOS").is_some() {
        // diagnostic local seulement : jamais écrit dans le rapport
        let mut v: Vec<_> = table.iter().collect();
        v.sort_by_key(|(_, j)| j.to_string());
        for (p, j) in v {
            eprintln!("{j}\t{p}");
        }
    }
    let anonymiseur = Anonymiseur::new(&table, &redact);
    let anon = |s: &str| sonde.anonymiser(s, &anonymiseur, &garder);

    let mut out = String::new();
    let total_lignes: u64 = sonde.fichiers.iter().map(|f| f.lignes).sum();
    let total_octets: u64 = sonde.fichiers.iter().map(|f| f.taille).sum();

    out.push_str("# Rapport du journal de Star Citizen (anonymisé)\n\n");
    out.push_str("> Produit par `companion/tools/sclog-probe` (CES Compagnon, étape 1). Lecture seule des fichiers écrits par le\n");
    out.push_str("> jeu, en flux. Les exemples sont anonymisés : pseudos → `<JOUEUR_n>` (`<JOUEUR_LOCAL>` = le joueur de ce PC),\n");
    out.push_str("> identifiants numériques → `<ID>`, GUID → `<GUID>`, IP → `<IP>`, valeurs hexadécimales → `<HEX>` / `<ADR>`,\n");
    out.push_str("> chemins utilisateur → `<UTILISATEUR>`. Les numéros de build du jeu sont conservés.\n\n");

    out.push_str("## Journaux analysés\n\n");
    out.push_str("| Fichier | Build | Version | Début (UTC) | Fin (UTC) | Taille | Lignes |\n|---|---|---|---|---|---|---|\n");
    for f in &sonde.fichiers {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            md(&anon(&f.nom)),
            f.build.as_deref().unwrap_or("?"),
            f.version.as_deref().unwrap_or("?"),
            f.debut.as_deref().unwrap_or("?"),
            f.fin.as_deref().unwrap_or("?"),
            taille_lisible(f.taille),
            f.lignes
        ));
    }
    out.push_str(&format!(
        "\n**Total** : {} fichier(s), {}, {} lignes, {} types de lignes distincts, {} pseudos distincts repérés (masqués).\n\n",
        sonde.fichiers.len(),
        taille_lisible(total_octets),
        total_lignes,
        sonde.types.len(),
        table.len()
    ));

    let shards: BTreeMap<String, u64> = sonde.fichiers.iter().flat_map(|f| f.shards.clone()).fold(BTreeMap::new(), |mut acc, (k, v)| {
        *acc.entry(k).or_default() += v;
        acc
    });
    if !shards.is_empty() {
        out.push_str("Serveurs (« shards ») rejoints : ");
        out.push_str(&shards.keys().map(|k| format!("`{}`", md(&anon(k)))).collect::<Vec<_>>().join(", "));
        out.push_str("\n\n");
    }

    out.push_str("## Structure d'une ligne\n\n");
    out.push_str("```\n<horodatage ISO UTC> [Niveau] <Étiquette> texte libre … [Team_X][Sous-système]\n```\n\n");
    let mut avec_etiquette = 0u64;
    let mut suites = 0u64;
    let mut sans = 0u64;
    for (k, s) in &sonde.types {
        if k.starts_with("(suite de ligne)") {
            suites += s.count;
        } else if k.contains("(sans") {
            sans += s.count;
        } else {
            avec_etiquette += s.count;
        }
    }
    out.push_str(&format!(
        "- avec étiquette `<…>` : **{}** lignes ({:.1} %)\n- horodatées sans étiquette : **{}** ({:.1} %)\n- suites de ligne (sans horodatage, blocs multi-lignes) : **{}** ({:.1} %)\n\n",
        avec_etiquette,
        100.0 * avec_etiquette as f64 / total_lignes.max(1) as f64,
        sans,
        100.0 * sans as f64 / total_lignes.max(1) as f64,
        suites,
        100.0 * suites as f64 / total_lignes.max(1) as f64
    ));

    // Tri : par nombre décroissant.
    let mut tous: Vec<(&String, &TypeStats)> = sonde.types.iter().collect();
    tous.sort_by(|a, b| b.1.count.cmp(&a.1.count).then(a.0.cmp(b.0)));

    out.push_str("## Types utiles aux objectifs du compagnon (exemples détaillés)\n\n");
    out.push_str("Sélection automatique par mots-clés (décès, serveur, quantique, mission, boutique, minage…). Tous les exemples sont anonymisés.\n\n");
    let mut par_cat: BTreeMap<&str, Vec<(&String, &TypeStats)>> = BTreeMap::new();
    for (k, s) in &tous {
        if k.starts_with("(suite de ligne)") {
            continue;
        }
        for c in categories(k) {
            par_cat.entry(c).or_default().push((k, s));
        }
    }
    for (cat, liste) in &par_cat {
        out.push_str(&format!("### {}\n\n", cat));
        for (k, s) in liste {
            out.push_str(&format!(
                "**`{}`** — {} ligne(s), dans {} fichier(s){}\n\n",
                md(&anon(k)),
                s.count,
                s.fichiers.len(),
                match (&s.premier, &s.dernier) {
                    (Some(a), Some(b)) => format!(", du {} au {}", a, b),
                    _ => String::new(),
                }
            ));
            out.push_str("```\n");
            for ex in &s.exemples {
                out.push_str(&anon(ex));
                out.push('\n');
            }
            out.push_str("```\n\n");
        }
    }

    out.push_str("## Tous les types à étiquette (nombre, un exemple)\n\n");
    out.push_str("| Type | Nombre | Fichiers | Exemple anonymisé |\n|---|---:|---:|---|\n");
    for (k, s) in &tous {
        if k.contains("(sans") || k.starts_with("(suite de ligne)") {
            continue;
        }
        let ex = s.exemples.first().map(|e| anon(e)).unwrap_or_default();
        let ex: String = ex.chars().take(220).collect();
        out.push_str(&format!("| `{}` | {} | {} | `{}` |\n", md(&anon(k)), s.count, s.fichiers.len(), md(&ex)));
    }

    out.push_str(&format!("\n## Lignes sans étiquette et suites de ligne ({} groupes les plus fréquents)\n\n", top_autres));
    out.push_str("| Groupe (3 premiers mots, chiffres → N) | Nombre | Exemple anonymisé |\n|---|---:|---|\n");
    for (k, s) in tous.iter().filter(|(k, _)| k.contains("(sans") || k.starts_with("(suite de ligne)")).take(top_autres) {
        let ex = s.exemples.first().map(|e| anon(e)).unwrap_or_default();
        let ex: String = ex.chars().take(160).collect();
        out.push_str(&format!("| `{}` | {} | `{}` |\n", md(&anon(k)), s.count, md(&ex)));
    }

    match &analyse {
        Some(p) => {
            let texte = std::fs::read_to_string(p).expect("lecture du fichier d'analyse");
            out.push('\n');
            out.push_str(&texte);
        }
        None => out.push_str("\n## Analyse\n\n_(à rédiger après lecture des résultats ; voir l'option --analyse)_\n"),
    }

    let mut f = File::create(&sortie).expect("création du rapport");
    f.write_all(out.as_bytes()).expect("écriture du rapport");
    eprintln!(
        "rapport : {} ({} types, {} pseudos masqués, joueur local {})",
        sortie.display(),
        sonde.types.len(),
        table.len(),
        if sonde.pseudo_local.is_some() { "repéré" } else { "non repéré" }
    );
}
