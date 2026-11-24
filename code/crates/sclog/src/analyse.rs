//! Analyse : état de présence et événements du joueur local (types du contrat d'API v1.1, § 5).
//!
//! Formats observés dans de vrais journaux 4.7 → 4.10 (voir `docs/companion/log-report.md`). Une ligne non
//! reconnue est ignorée, jamais une erreur : le format du jeu change d'une version à l'autre.

use crate::ligne::Ligne;
use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;

/// État de présence déduit du JOURNAL seul. L'application le combine avec le processus (jeu lancé ou non).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Presence {
    /// Pas de session en cours dans le journal (ou session terminée par `<SystemQuit>`).
    #[default]
    Offline,
    /// Journal commencé, compte pas encore connecté.
    Launching,
    /// Compte connecté, au menu principal.
    Menu,
    /// Dans l'univers persistant.
    Universe,
}

/// Événement du joueur local. `decalage` = position de la ligne dans le fichier (identifiant stable).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum Evenement {
    #[serde(rename = "session.start")]
    SessionDebut { at: String, decalage: u64 },
    #[serde(rename = "session.end")]
    SessionFin { at: String, decalage: u64, started_at: Option<String> },
    #[serde(rename = "presence")]
    Presence { at: String, decalage: u64, state: Presence, region: Option<String> },
    #[serde(rename = "vehicle.destroyed")]
    VaisseauDetruit { at: String, decalage: u64, vehicle_class: String, reason: String, role: String, level: u8 },
    #[serde(rename = "player.down")]
    JoueurATerre { at: String, decalage: u64, kind: String },
    #[serde(rename = "travel.quantum")]
    SautQuantique { at: String, decalage: u64, vehicle_class: String, destination: Option<String> },
    /// Carnet local : route quantique calculée (départ → destination, noms lisibles).
    #[serde(rename = "travel.route")]
    RouteCalculee { at: String, decalage: u64, depart: String, destination: String },
    /// Carnet local : nouvelle juridiction (texte du HUD).
    #[serde(rename = "travel.jurisdiction")]
    Juridiction { at: String, decalage: u64, nom: String },
    /// Carnet local : entrée (`true`) ou sortie d'une zone d'armistice.
    #[serde(rename = "travel.armistice")]
    Armistice { at: String, decalage: u64, entree: bool },
    #[serde(rename = "mission.completed")]
    MissionTerminee { at: String, decalage: u64 },
}

impl Evenement {
    pub fn at(&self) -> &str {
        match self {
            Evenement::SessionDebut { at, .. }
            | Evenement::SessionFin { at, .. }
            | Evenement::Presence { at, .. }
            | Evenement::VaisseauDetruit { at, .. }
            | Evenement::JoueurATerre { at, .. }
            | Evenement::SautQuantique { at, .. }
            | Evenement::RouteCalculee { at, .. }
            | Evenement::Juridiction { at, .. }
            | Evenement::Armistice { at, .. }
            | Evenement::MissionTerminee { at, .. } => at,
        }
    }
}

struct Motifs {
    build: Regex,
    version: Regex,
    geid: Regex,
    perso: Regex,
    shard: Regex,
    detruit: Regex,
    quantique: Regex,
    fin_mission: Regex,
    incapacite: Regex,
    route: Regex,
    juridiction: Regex,
}

fn motifs() -> &'static Motifs {
    static M: OnceLock<Motifs> = OnceLock::new();
    M.get_or_init(|| Motifs {
        build: Regex::new(r"Build\((\d+)\)").unwrap(),
        version: Regex::new(r"^FileVersion: ([\d.]+)").unwrap(),
        geid: Regex::new(r"playerGEID=(\d+)").unwrap(),
        perso: Regex::new(r"geid (\d+) - accountId \d+ - name \S+ - state STATE_CURRENT").unwrap(),
        shard: Regex::new(r"shard\[[a-z]+_([a-z]+)\d").unwrap(),
        detruit: Regex::new(
            r"class '([^']+)' \(owner player \[(\d+)\].*?destroyed: reason \[([^\]]+)\] level \[\d\] -> \[(\d)\].*?killer '[^']*' \[(\d+)\]",
        )
        .unwrap(),
        quantique: Regex::new(r"\| ([A-Za-z0-9_]+?)_\d{6,}\[\d+\]\|CSCItemNavigation::OnQuantumDriveArrived").unwrap(),
        fin_mission: Regex::new(r"PlayerId\[(\d+)\] CompletionType\[(\w+)\]").unwrap(),
        incapacite: Regex::new(r#"Added notification "(INCAPACIT|Incapacitat)"#).unwrap(),
        route: Regex::new(r"Projected Start Location is (.+?) for route to destination (\S+)").unwrap(),
        juridiction: Regex::new(r#"Added notification "(?:JURIDICTION\s*:\s*([^:"]+)|Entered (.+?) Jurisdiction)"#).unwrap(),
    })
}

/// Délai sous lequel deux signaux « à terre » sont considérés comme le même (une mort écrit des dizaines de lignes).
const ANTI_REBOND_SEC: i64 = 90;

#[derive(Default)]
pub struct Analyseur {
    presence: Presence,
    region: Option<String>,
    geid_local: Option<String>,
    debut_session: Option<String>,
    build: Option<String>,
    version: Option<String>,
    dernier_a_terre: Option<i64>,
    derniere_destination: Option<String>,
    derniere_juridiction: Option<String>,
    en_armistice: Option<bool>,
}

impl Analyseur {
    pub fn nouveau() -> Self {
        Self::default()
    }

    /// Nouveau fichier (rotation) : tout ce qui concernait la session précédente est oublié.
    pub fn nouveau_fichier(&mut self) {
        *self = Self::default();
    }

    pub fn presence(&self) -> Presence {
        self.presence
    }
    pub fn region(&self) -> Option<&str> {
        self.region.as_deref()
    }
    pub fn debut_session(&self) -> Option<&str> {
        self.debut_session.as_deref()
    }
    pub fn build(&self) -> Option<&str> {
        self.build.as_deref()
    }
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    fn changer(&mut self, at: &str, decalage: u64, p: Presence, sortie: &mut Vec<Evenement>) {
        if self.presence != p {
            self.presence = p;
            if p != Presence::Universe {
                self.region = None;
            }
            sortie.push(Evenement::Presence { at: at.to_string(), decalage, state: p, region: self.region.clone() });
        }
    }

    /// Traite une ligne ; renvoie les événements qu'elle produit (souvent aucun).
    pub fn traiter(&mut self, decalage: u64, brute: &str) -> Vec<Evenement> {
        let mut sortie = Vec::new();
        let l = Ligne::decouper(brute);
        let Some(at) = l.horodatage else { return sortie };
        let m = motifs();

        match l.etiquette {
            None => {
                if l.texte.starts_with("Log started") {
                    // Le build (`BackupNameAttachment`) est écrit juste AVANT « Log started » : on le garde.
                    let build = self.build.take();
                    self.nouveau_fichier();
                    self.build = build;
                    self.debut_session = Some(at.to_string());
                    self.changer(at, decalage, Presence::Launching, &mut sortie);
                    sortie.insert(0, Evenement::SessionDebut { at: at.to_string(), decalage });
                } else if self.build.is_none() && l.texte.contains("BackupNameAttachment") {
                    self.build = m.build.captures(l.texte).map(|c| c[1].to_string());
                } else if self.version.is_none() {
                    if let Some(c) = m.version.captures(l.texte) {
                        self.version = Some(c[1].to_string());
                    }
                }
            }
            Some("Legacy login response") if l.texte.contains("Login Success") => {
                // Une reconnexion EN PLEIN univers (observée) ne ramène pas au menu.
                if matches!(self.presence, Presence::Launching | Presence::Offline) {
                    self.changer(at, decalage, Presence::Menu, &mut sortie);
                }
            }
            Some("Expect Incoming Connection") | Some("Channel Created") => {
                if self.geid_local.is_none() {
                    self.geid_local = m.geid.captures(l.texte).map(|c| c[1].to_string());
                }
            }
            Some("AccountLoginCharacterStatus_Character") => {
                if let Some(c) = m.perso.captures(l.texte) {
                    self.geid_local = Some(c[1].to_string());
                }
            }
            Some("Join PU") => {
                self.derniere_juridiction = None;
                self.en_armistice = None;
                self.region = m.shard.captures(l.texte).map(|c| c[1].to_string());
                if self.presence == Presence::Universe {
                    // changement de serveur : on renvoie la présence avec la nouvelle région
                    sortie.push(Evenement::Presence { at: at.to_string(), decalage, state: Presence::Universe, region: self.region.clone() });
                } else {
                    let r = self.region.clone();
                    self.changer(at, decalage, Presence::Universe, &mut sortie);
                    self.region = r;
                    if let Some(Evenement::Presence { region, .. }) = sortie.last_mut() {
                        *region = self.region.clone();
                    }
                }
            }
            Some("Channel Disconnected") if l.texte.contains("Player requested disconnect") => {
                if self.presence == Presence::Universe {
                    self.changer(at, decalage, Presence::Menu, &mut sortie);
                }
            }
            Some("SystemQuit") => {
                self.changer(at, decalage, Presence::Offline, &mut sortie);
                if let Some(debut) = self.debut_session.take() {
                    sortie.push(Evenement::SessionFin { at: at.to_string(), decalage, started_at: Some(debut) });
                }
            }
            Some("Vehicle Destroyed") => {
                if let (Some(moi), Some(c)) = (&self.geid_local, m.detruit.captures(l.texte)) {
                    let proprio = &c[2] == moi;
                    let auteur = &c[5] == moi;
                    if proprio || auteur {
                        let role = match (proprio, auteur) {
                            (true, true) => "both",
                            (true, false) => "owner",
                            _ => "killer",
                        };
                        let reason = match &c[3] {
                            "Combat" => "combat",
                            "Collision" => "collision",
                            _ => "other",
                        };
                        sortie.push(Evenement::VaisseauDetruit {
                            at: at.to_string(),
                            decalage,
                            vehicle_class: c[1].chars().take(80).collect(),
                            reason: reason.to_string(),
                            role: role.to_string(),
                            level: c[4].parse().unwrap_or(0),
                        });
                    }
                }
            }
            Some("Calculate Route") => {
                if let Some(c) = m.route.captures(l.texte) {
                    let destination = crate::lieux::lisible(&c[2]);
                    if self.derniere_destination.as_deref() != Some(destination.as_str()) {
                        sortie.push(Evenement::RouteCalculee {
                            at: at.to_string(),
                            decalage,
                            depart: c[1].trim().chars().take(80).collect(),
                            destination: destination.clone(),
                        });
                    }
                    self.derniere_destination = Some(destination);
                }
            }
            Some("SHUDEvent_OnNotification") if m.juridiction.is_match(l.texte) => {
                if let Some(c) = m.juridiction.captures(l.texte) {
                    let nom = crate::lieux::juridiction(c.get(1).or(c.get(2)).map(|x| x.as_str()).unwrap_or(""));
                    if !nom.is_empty() && self.derniere_juridiction.as_deref() != Some(nom.as_str()) {
                        self.derniere_juridiction = Some(nom.clone());
                        sortie.push(Evenement::Juridiction { at: at.to_string(), decalage, nom: nom.chars().take(80).collect() });
                    }
                }
            }
            Some("SHUDEvent_OnNotification")
                if l.texte.contains("ZONE D'ARMISTICE") || l.texte.contains("Armistice Zone") =>
            {
                let entree = l.texte.contains("VOUS ENTREZ") || l.texte.contains("Entering");
                if self.en_armistice != Some(entree) {
                    self.en_armistice = Some(entree);
                    sortie.push(Evenement::Armistice { at: at.to_string(), decalage, entree });
                }
            }
            Some(e) if e.contains("CSCActorCorpseUtils") => self.a_terre(at, decalage, "dead", &mut sortie),
            Some("SHUDEvent_OnNotification") if m.incapacite.is_match(l.texte) => {
                self.a_terre(at, decalage, "incapacitated", &mut sortie)
            }
            Some(e) if e.starts_with("Quantum Drive Arrived") => {
                if let Some(c) = m.quantique.captures(l.texte) {
                    sortie.push(Evenement::SautQuantique {
                        at: at.to_string(),
                        decalage,
                        vehicle_class: c[1].chars().take(80).collect(),
                        destination: self.derniere_destination.take(),
                    });
                }
            }
            Some("EndMission") => {
                if let (Some(moi), Some(c)) = (&self.geid_local, m.fin_mission.captures(l.texte)) {
                    if &c[1] == moi && &c[2] == "Complete" {
                        sortie.push(Evenement::MissionTerminee { at: at.to_string(), decalage });
                    }
                }
            }
            _ => {}
        }
        sortie
    }

    /// Le processus du jeu a disparu sans `<SystemQuit>` (plantage, fin de tâche) : on clôt la session ouverte.
    /// `decalage` = `u64::MAX` (aucune ligne du journal ne porte cette fin).
    pub fn fermer_session(&mut self, at: &str) -> Vec<Evenement> {
        let mut sortie = Vec::new();
        if let Some(debut) = self.debut_session.take() {
            self.changer(at, u64::MAX, Presence::Offline, &mut sortie);
            sortie.push(Evenement::SessionFin { at: at.to_string(), decalage: u64::MAX, started_at: Some(debut) });
        }
        sortie
    }

    fn a_terre(&mut self, at: &str, decalage: u64, kind: &str, sortie: &mut Vec<Evenement>) {
        let t = secondes(at);
        if let (Some(t), Some(avant)) = (t, self.dernier_a_terre) {
            if t - avant < ANTI_REBOND_SEC {
                return;
            }
        }
        self.dernier_a_terre = t;
        sortie.push(Evenement::JoueurATerre { at: at.to_string(), decalage, kind: kind.to_string() });
    }
}

/// Secondes approximatives depuis une origine fixe, pour comparer deux horodatages ISO proches.
fn secondes(iso: &str) -> Option<i64> {
    // 2026-11-13T16:46:23.938Z
    let b = iso.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let n = |a: usize, z: usize| iso.get(a..z)?.parse::<i64>().ok();
    let (j, h, mi, s) = (n(8, 10)?, n(11, 13)?, n(14, 16)?, n(17, 19)?);
    let mois = n(5, 7)?;
    let an = n(0, 4)?;
    Some((((an * 372 + mois * 31 + j) * 24 + h) * 60 + mi) * 60 + s)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Lignes au format réel (4.10), pseudos et identifiants inventés.
    const MOI: &str = "111111111111";
    fn journal() -> Vec<String> {
        vec![
            "<2026-11-13T16:42:23.124Z> BackupNameAttachment=\" Build(13165946) 13 Nov 26 (17 42 17)\"  -- used by backup system".into(),
            "<2026-11-13T16:42:23.124Z> Log started on Fri Nov 13 16:42:23 2026".into(),
            "<2026-11-13T16:42:23.124Z> FileVersion: 4.10.200.58746".into(),
            "<2026-11-13T16:42:36.729Z> [Notice] <AccountLoginCharacterStatus_Character> Character: createdAt 1 - updatedAt 2 - geid 111111111111 - accountId 9 - name Moi - state STATE_CURRENT [Team_GameServices][Login]".into(),
            "<2026-11-13T16:42:38.189Z> [Notice] <Legacy login response> [CIG-net] User Login Success - Handle[Moi] - Time[1] [Team_GameServices][Login]".into(),
            "<2026-11-13T16:46:23.938Z> [Notice] <Join PU> address[1.2.3.4] port[64298] shard[pub_euw1b_13165946_200] locationId[-1] [Team_GameServices][GIM][Matchmaking]".into(),
            "<2026-11-13T17:07:31.465Z> [Notice] <Vehicle Destroyed> Vehicle 'DRAK_Cutter_Rambler_930058506774' [930058506774] class 'DRAK_Cutter_Rambler' (owner player [111111111111], driver [0]) destroyed: reason [Collision] level [0] -> [2] flags [4066] killer 'Moi' [111111111111] authority [0] [Team_CGP4][Vehicle]".into(),
            "<2026-11-13T17:54:20.097Z> [Notice] <Vehicle Destroyed> Vehicle 'CRUS_Intrepid_929652160923' [929652160923] class 'CRUS_Intrepid' (owner player [222222222222], driver [0]) destroyed: reason [Combat] level [0] -> [2] flags [4066] killer 'Autre' [222222222222] authority [0] [Team_CGP4][Vehicle]".into(),
            "<2026-11-13T18:00:00.000Z> [Notice] <Vehicle Destroyed> Vehicle 'AEGS_Gladius_1234567' [1234567] class 'AEGS_Gladius' (owner player [333333333333], driver [0]) destroyed: reason [Combat] level [0] -> [2] flags [1] killer 'Moi' [111111111111] authority [0] [Team_CGP4][Vehicle]".into(),
            "<2026-11-13T18:30:00.000Z> [Notice] <Calculate Route> [ItemNavigation][CL][4464] | NOT AUTH | DRAK_Ironclad_929965782697[929965782697]|CSCItemNavigation::CalculateRoute|Projected Start Location is microTech for route to destination OOC_Stanton_4b_Clio [Team_CGP4][QuantumTravel]".into(),
            "<2026-11-13T18:31:00.000Z> [Notice] <SHUDEvent_OnNotification> Added notification \"JURIDICTION : microTech: \" [12] to queue. New queue size: 1 [Team_CoreGameplayFeatures]".into(),
            "<2026-11-13T18:31:01.000Z> [Notice] <SHUDEvent_OnNotification> Added notification \"JURIDICTION : microTech: \" [12] to queue. New queue size: 2 [Team_CoreGameplayFeatures]".into(),
            "<2026-11-13T18:32:00.000Z> [Notice] <SHUDEvent_OnNotification> Added notification \"VOUS QUITTEZ LA ZONE D'ARMISTICE - PRUDENCE EST MÈRE DE SÛRETÉ: \" [13] to queue. [Team_CoreGameplayFeatures]".into(),
            "<2026-11-13T18:34:24.662Z> [Notice] <Quantum Drive Arrived - Arrived at Final Destination> [ItemNavigation][CL][33580] | NOT AUTH | DRAK_Ironclad_929965782697[929965782697]|CSCItemNavigation::OnQuantumDriveArrived|Quantum Drive has arrived at final destination [Team_CGP4][QuantumTravel]".into(),
            "<2026-11-13T19:40:59.245Z> [Notice] <EndMission> Ending mission for player. MissionId[aaaa] Player[Moi] PlayerId[111111111111] CompletionType[Complete] Reason[Mission Ended] [Team_MissionFeatures][Missions]".into(),
            "<2026-11-13T21:25:08.633Z> [Notice] <EndMission> Ending mission for player. MissionId[bbbb] Player[Moi] PlayerId[111111111111] CompletionType[Abandon] Reason[Player left] [Team_MissionFeatures][Missions]".into(),
            "<2026-11-13T22:00:00.000Z> [Notice] <Adding non kept item [CSCActorCorpseUtils::PopulateItemPortForItemRecoveryEntitlement]> Item 'body_01' [Team_CoreGameplayFeatures]".into(),
            "<2026-11-13T22:00:01.000Z> [Notice] <Adding non kept item [CSCActorCorpseUtils::PopulateItemPortForItemRecoveryEntitlement]> Item 'helmet' [Team_CoreGameplayFeatures]".into(),
            "<2026-11-13T20:19:37.176Z> [Notice] <Legacy login response> [CIG-net] User Login Success - Handle[Moi] - Time[2] [Team_GameServices][Login]".into(),
            "<2026-11-14T01:13:37.117Z> [Notice] <Channel Disconnected> cause=30016 reason=\"Remote Disconnect - Player requested disconnect\" frame=1 [Team_Network]".into(),
            "<2026-11-14T01:13:37.167Z> [Notice] <SystemQuit> CSystem::Quit invoked with - cause=30016, reason=Quit via console command, exitCode=0 [Team_Unknown][System]".into(),
        ]
    }

    fn analyser() -> (Analyseur, Vec<Evenement>) {
        let mut a = Analyseur::nouveau();
        let mut tout = Vec::new();
        for (i, l) in journal().iter().enumerate() {
            tout.extend(a.traiter(i as u64 * 100, l));
        }
        (a, tout)
    }

    #[test]
    fn deroule_une_session() {
        let (a, ev) = analyser();
        let types: Vec<String> = ev.iter().map(|e| serde_json_type(e)).collect();
        assert_eq!(
            types,
            vec![
                "session.start", "presence:launching", "presence:menu", "presence:universe(euw)",
                "vehicle.destroyed:owner/both", "vehicle.destroyed:killer",
                "travel.route:Clio", "travel.jurisdiction:microTech", "travel.armistice:false", "travel.quantum:Clio",
                "mission.completed",
                "player.down", "presence:menu", "presence:offline", "session.end",
            ]
        );
        assert_eq!(a.build(), Some("13165946"));
        assert_eq!(a.version(), Some("4.10.200.58746"));
        assert_eq!(a.presence(), Presence::Offline);
        let _ = MOI;
    }

    #[test]
    fn aucun_tiers_dans_les_evenements() {
        let (_, ev) = analyser();
        let json = format!("{:?}", ev);
        for interdit in ["Autre", "222222222222", "333333333333", "1.2.3.4", "Moi", "111111111111"] {
            assert!(!json.contains(interdit), "{interdit} ne doit pas sortir de l'analyse");
        }
    }

    #[test]
    fn plantage_ferme_la_session_une_seule_fois() {
        let mut a = Analyseur::nouveau();
        a.traiter(0, "<2026-11-13T16:42:23.124Z> Log started on Fri Nov 13 16:42:23 2026");
        let ev = a.fermer_session("2026-11-13T17:00:00.000Z");
        assert!(matches!(ev.last(), Some(Evenement::SessionFin { started_at: Some(d), .. }) if d == "2026-11-13T16:42:23.124Z"));
        assert!(a.fermer_session("2026-11-13T17:00:05.000Z").is_empty());
        assert_eq!(a.presence(), Presence::Offline);
    }

    fn serde_json_type(e: &Evenement) -> String {
        match e {
            Evenement::SessionDebut { .. } => "session.start".into(),
            Evenement::SessionFin { .. } => "session.end".into(),
            Evenement::Presence { state, region, .. } => match region {
                Some(r) => format!("presence:{:?}({r})", state).to_lowercase(),
                None => format!("presence:{:?}", state).to_lowercase(),
            },
            Evenement::VaisseauDetruit { role, .. } if role == "both" => "vehicle.destroyed:owner/both".into(),
            Evenement::VaisseauDetruit { role, .. } => format!("vehicle.destroyed:{role}"),
            Evenement::JoueurATerre { .. } => "player.down".into(),
            Evenement::SautQuantique { destination, .. } => format!("travel.quantum:{}", destination.clone().unwrap_or_default()),
            Evenement::RouteCalculee { destination, .. } => format!("travel.route:{destination}"),
            Evenement::Juridiction { nom, .. } => format!("travel.jurisdiction:{nom}"),
            Evenement::Armistice { entree, .. } => format!("travel.armistice:{entree}"),
            Evenement::MissionTerminee { .. } => "mission.completed".into(),
        }
    }
}
