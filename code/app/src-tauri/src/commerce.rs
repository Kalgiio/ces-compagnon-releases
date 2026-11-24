//! Routes de commerce (feuille de route § 3, décision du CES n° 5 : API publique d'UEX, créditée, sans clé).
//!
//! - Jeton d'application UEX FACULTATIF, propre à chaque membre (0.14.0, décision du CES) : créé par le membre sur
//!   uexcorp.space/api/apps, collé dans l'application, rangé dans le gestionnaire d'identification de Windows, envoyé
//!   en `Authorization: Bearer`. Il donne un quota personnel (172 800 requêtes / jour). Jamais dans le code : si UEX
//!   change ou révoque un jeton, le membre en colle un autre, sans nouvelle version. Sans jeton : points publics.
//! - Données : UEX Corp, API 2.0 (`https://api.uexcorp.uk/2.0/`), points publics sans jeton. Limite annoncée :
//!   120 requêtes/min ; on en fait 4 au plus par rafraîchissement, avec un cache local sur disque (prix : 30 min,
//!   terminaux / vaisseaux / marchandises : 12 h). JAMAIS de lecture du jeu pour les prix.
//! - Vaisseaux du membre : `GET /api/companion/hangar` du portail (soute du wiki), sinon soute UEX.
//! - Le calcul des routes se fait sur le PC : soute, budget, système, départ, âge maximal des prix.

use crate::portail;
use crate::presence::{jeton_actuel, Portail};
use crate::traduction::Dossiers;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const UEX: &str = "https://api.uexcorp.uk/2.0";
const SERVICE_UEX: &str = "CES Compagnon — UEX";

fn entree_uex() -> Option<keyring::Entry> {
    keyring::Entry::new(SERVICE_UEX, "jeton").ok()
}

/// Jeton UEX du membre, s'il en a enregistré un.
fn jeton_uex() -> Option<String> {
    entree_uex()?.get_password().ok().filter(|j| !j.is_empty())
}

/// Format d'un jeton d'application UEX : 40 caractères hexadécimaux (contrôle avant tout envoi).
fn jeton_valide(j: &str) -> bool {
    j.len() == 40 && j.chars().all(|c| c.is_ascii_hexdigit())
}

fn requete_uex(point: &str, jeton: Option<&str>) -> Result<Value, String> {
    let mut req = portail::client().get(format!("{UEX}/{point}")).timeout(Duration::from_secs(60));
    if let Some(j) = jeton {
        req = req.bearer_auth(j);
    }
    req.send().map_err(|e| e.to_string()).and_then(|r| r.json::<Value>().map_err(|e| e.to_string()))
}
const TTL_PRIX: u64 = 30 * 60;
const TTL_REFERENTIEL: u64 = 12 * 3600;

fn maintenant() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Serialize, Deserialize)]
struct Cache {
    recu: u64,
    data: Value,
}

/// Lit `point` depuis le cache disque s'il est assez récent, sinon le télécharge. Renvoie (données, reçu à).
pub(crate) fn charger(dossiers: &Dossiers, point: &str, ttl: u64, forcer: bool) -> Result<(Value, u64), String> {
    let fichier = dossiers.donnees.join("cache").join(format!("uex-{point}.json"));
    let ancien: Option<Cache> = std::fs::read(&fichier).ok().and_then(|b| serde_json::from_slice(&b).ok());
    if let Some(c) = &ancien {
        if !forcer && maintenant().saturating_sub(c.recu) < ttl {
            return Ok((c.data.clone(), c.recu));
        }
    }
    // Avec le jeton du membre s'il en a un ; s'il est refusé, nouvel essai sans jeton (points publics).
    let jeton = jeton_uex();
    let mut telecharge = requete_uex(point, jeton.as_deref());
    if jeton.is_some() && !matches!(&telecharge, Ok(v) if v["status"] == "ok") {
        telecharge = requete_uex(point, None);
    }
    match telecharge {
        Ok(v) if v["status"] == "ok" && v["data"].is_array() => {
            let c = Cache { recu: maintenant(), data: v["data"].clone() };
            if let Some(d) = fichier.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::write(&fichier, serde_json::to_vec(&c).unwrap_or_default());
            Ok((c.data, c.recu))
        }
        // UEX indisponible ou limite atteinte : on se rabat sur le cache, même ancien.
        resultat => match ancien {
            Some(c) => Ok((c.data, c.recu)),
            None => Err(match resultat {
                Ok(v) => format!("UEX : {}", v["status"].as_str().unwrap_or("réponse inattendue")),
                Err(e) => format!("UEX injoignable : {e}"),
            }),
        },
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EtatJetonUex {
    present: bool,
}

#[tauri::command]
pub fn uex_jeton_etat() -> EtatJetonUex {
    EtatJetonUex { present: jeton_uex().is_some() }
}

/// Vérifie le jeton auprès d'UEX, puis le range dans le gestionnaire d'identification de Windows.
#[tauri::command]
pub async fn uex_jeton_enregistrer(jeton: String) -> Result<(), String> {
    let j = jeton.trim().to_ascii_lowercase();
    if !jeton_valide(&j) {
        return Err("Ce n'est pas un jeton UEX : il fait 40 caractères (chiffres et lettres a à f).".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let v = requete_uex("commodities", Some(&j)).map_err(|e| format!("UEX injoignable : {e}"))?;
        if v["status"] != "ok" {
            return Err(format!("UEX refuse ce jeton ({}).", v["status"].as_str().unwrap_or("réponse inattendue")));
        }
        entree_uex().ok_or("Gestionnaire d'identification indisponible.")?.set_password(&j).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn uex_jeton_effacer() {
    if let Some(e) = entree_uex() {
        let _ = e.delete_credential();
    }
}

/// Page UEX où chacun crée son application et récupère son jeton.
#[tauri::command]
pub fn uex_ouvrir_page(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url("https://uexcorp.space/api/apps", None::<&str>).map_err(|e| e.to_string())
}

fn nombre(v: &Value) -> f64 {
    v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0.0)
}
fn texte(v: &Value) -> Option<String> {
    v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

#[derive(Clone)]
struct Terminal {
    nom: String,
    lieu: String,
    systeme: String,
    disponible: bool,
    max_conteneur: f64,
}

fn terminaux(data: &Value) -> HashMap<i64, Terminal> {
    let mut m = HashMap::new();
    for t in data.as_array().into_iter().flatten() {
        if t["type"] != "commodity" {
            continue;
        }
        let Some(id) = t["id"].as_i64() else { continue };
        let nom = texte(&t["displayname"]).or_else(|| texte(&t["nickname"])).or_else(|| texte(&t["name"])).unwrap_or_default();
        // Lieu lisible : lune, sinon planète ; station / ville / avant-poste en complément.
        let corps = texte(&t["moon_name"]).or_else(|| texte(&t["planet_name"])).or_else(|| texte(&t["orbit_name"]));
        let site = texte(&t["space_station_name"]).or_else(|| texte(&t["city_name"])).or_else(|| texte(&t["outpost_name"]));
        // On écarte ce qui répète le nom du terminal (« Pyro Gateway · Pyro Gateway (Stanton) · … »).
        let n = nom.to_lowercase();
        let utile = |x: &String| {
            let x = x.to_lowercase();
            !x.contains(&n) && !n.contains(&x)
        };
        let mut parties: Vec<String> = Vec::new();
        for x in [site, corps].into_iter().flatten() {
            if utile(&x) && !parties.iter().any(|p| p.to_lowercase().contains(&x.to_lowercase())) {
                parties.push(x);
            }
        }
        let lieu = parties.join(" · ");
        m.insert(
            id,
            Terminal {
                nom,
                lieu,
                systeme: texte(&t["star_system_name"]).unwrap_or_else(|| "?".into()),
                disponible: nombre(&t["is_available_live"]) > 0.0 || t["is_available_live"].is_null(),
                max_conteneur: nombre(&t["max_container_size"]),
            },
        );
    }
    m
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Vaisseau {
    nom: String,
    scu: f64,
    /// `hangar` (vaisseau du membre) ou `uex` (tous les vaisseaux de commerce).
    source: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Donnees {
    vaisseaux: Vec<Vaisseau>,
    hangar_erreur: Option<String>,
    systemes: Vec<String>,
    terminaux: Vec<Value>,
    prix_recus_a: u64,
    nb_prix: usize,
}

fn cle_nom(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

#[tauri::command(async)]
pub fn commerce_donnees(
    dossiers: tauri::State<'_, Dossiers>,
    portail_etat: tauri::State<'_, Portail>,
    forcer: Option<bool>,
) -> Result<Donnees, String> {
    let forcer = forcer.unwrap_or(false);
    let (prix, recu) = charger(&dossiers, "commodities_prices_all", TTL_PRIX, forcer)?;
    let (term, _) = charger(&dossiers, "terminals", TTL_REFERENTIEL, false)?;
    let (veh, _) = charger(&dossiers, "vehicles", TTL_REFERENTIEL, false)?;
    let t = terminaux(&term);

    // Soutes UEX, indexées par nom normalisé (pour compléter le hangar quand le wiki n'a pas la soute).
    let mut uex: Vec<Vaisseau> = Vec::new();
    let mut par_nom: HashMap<String, f64> = HashMap::new();
    for v in veh.as_array().into_iter().flatten() {
        let scu = nombre(&v["scu"]);
        let nom = texte(&v["name_full"]).or_else(|| texte(&v["name"])).unwrap_or_default();
        if let Some(n) = texte(&v["name"]) {
            par_nom.insert(cle_nom(&n), scu);
        }
        par_nom.insert(cle_nom(&nom), scu);
        if scu > 0.0 && nombre(&v["is_ground_vehicle"]) == 0.0 {
            uex.push(Vaisseau { nom, scu, source: "uex".into() });
        }
    }
    uex.sort_by(|a, b| a.nom.cmp(&b.nom));

    // Hangar du membre (portail), si l'appareil est relié.
    let mut vaisseaux = Vec::new();
    let mut hangar_erreur = None;
    match jeton_actuel(&portail_etat) {
        Ok(j) => match portail::appel(&j, reqwest::Method::GET, "hangar", None) {
            Ok(h) => {
                for s in h["ships"].as_array().into_iter().flatten() {
                    let modele = texte(&s["model"]).unwrap_or_default();
                    let fabricant = texte(&s["manufacturer"]).unwrap_or_default();
                    let scu = s["cargoScu"].as_f64().or_else(|| {
                        par_nom.get(&cle_nom(&format!("{fabricant} {modele}"))).or_else(|| par_nom.get(&cle_nom(&modele))).copied()
                    });
                    let Some(scu) = scu.filter(|s| *s > 0.0) else { continue };
                    let perso = texte(&s["name"]).filter(|n| *n != modele);
                    let nom = match perso { Some(p) => format!("{p} ({modele})"), None => modele };
                    vaisseaux.push(Vaisseau { nom, scu, source: "hangar".into() });
                }
            }
            Err(e) => hangar_erreur = Some(e.to_string()),
        },
        Err(_) => hangar_erreur = Some("Relie ton compte pour voir les vaisseaux de ton hangar.".into()),
    }
    vaisseaux.extend(uex);

    let mut systemes: Vec<String> = t.values().map(|x| x.systeme.clone()).collect();
    systemes.sort();
    systemes.dedup();
    let mut liste: Vec<(i64, &Terminal)> = t.iter().map(|(i, x)| (*i, x)).collect();
    liste.sort_by(|a, b| (&a.1.systeme, &a.1.nom).cmp(&(&b.1.systeme, &b.1.nom)));
    let terminaux = liste
        .into_iter()
        .map(|(id, x)| serde_json::json!({ "id": id, "nom": x.nom, "lieu": x.lieu, "systeme": x.systeme }))
        .collect();
    Ok(Donnees {
        vaisseaux,
        hangar_erreur,
        systemes,
        terminaux,
        prix_recus_a: recu,
        nb_prix: prix.as_array().map_or(0, |a| a.len()),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parametres {
    scu: f64,
    budget: Option<f64>,
    systeme: Option<String>,
    depart: Option<i64>,
    illegales: Option<bool>,
    age_max_jours: Option<f64>,
    limite: Option<usize>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Etape {
    terminal: String,
    lieu: String,
    systeme: String,
    prix: f64,
    /// Stock disponible à l'achat, ou demande à la vente (SCU) ; 0 = inconnu.
    scu_dispo: f64,
    age_heures: f64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    marchandise: String,
    illegale: bool,
    achat: Etape,
    vente: Etape,
    scu: f64,
    investissement: f64,
    benefice: f64,
    marge_unitaire: f64,
    /// Plus petit conteneur maximal des deux terminaux (SCU), 0 = inconnu.
    conteneur_max: f64,
}

struct Ligne<'a> {
    commodite: i64,
    nom: String,
    terminal: i64,
    t: &'a Terminal,
    achat: f64,
    vente: f64,
    scu_achat: f64,
    scu_vente: f64,
    age_h: f64,
}

/// Calcul pur (testable) : meilleures routes achat → vente.
fn calculer(prix: &Value, t: &HashMap<i64, Terminal>, illegales: &[i64], p: &Parametres) -> Vec<Route> {
    let age_max_h = p.age_max_jours.unwrap_or(7.0) * 24.0;
    let now = maintenant() as f64;
    let mut lignes: Vec<Ligne> = Vec::new();
    for x in prix.as_array().into_iter().flatten() {
        let (Some(c), Some(ti)) = (x["id_commodity"].as_i64(), x["id_terminal"].as_i64()) else { continue };
        let Some(term) = t.get(&ti).filter(|z| z.disponible) else { continue };
        if !p.illegales.unwrap_or(false) && illegales.contains(&c) {
            continue;
        }
        if let Some(s) = &p.systeme {
            if &term.systeme != s {
                continue;
            }
        }
        let age_h = (now - nombre(&x["date_modified"])).max(0.0) / 3600.0;
        if age_h > age_max_h {
            continue;
        }
        lignes.push(Ligne {
            commodite: c,
            nom: texte(&x["commodity_name"]).unwrap_or_default(),
            terminal: ti,
            t: term,
            achat: nombre(&x["price_buy"]),
            vente: nombre(&x["price_sell"]),
            scu_achat: nombre(&x["scu_buy"]),
            scu_vente: nombre(&x["scu_sell"]),
            age_h,
        });
    }
    let etape = |l: &Ligne, prix: f64, dispo: f64| Etape {
        terminal: l.t.nom.clone(),
        lieu: l.t.lieu.clone(),
        systeme: l.t.systeme.clone(),
        prix,
        scu_dispo: dispo,
        age_heures: (l.age_h * 10.0).round() / 10.0,
    };
    let mut routes = Vec::new();
    for a in lignes.iter().filter(|l| l.achat > 0.0) {
        if p.depart.is_some_and(|d| d != a.terminal) {
            continue;
        }
        for v in lignes.iter().filter(|l| l.commodite == a.commodite && l.vente > a.achat && l.terminal != a.terminal) {
            let mut scu = p.scu.max(0.0);
            if let Some(b) = p.budget.filter(|b| *b > 0.0) {
                scu = scu.min((b / a.achat).floor());
            }
            if a.scu_achat > 0.0 {
                scu = scu.min(a.scu_achat);
            }
            if v.scu_vente > 0.0 {
                scu = scu.min(v.scu_vente);
            }
            let scu = scu.floor();
            if scu < 1.0 {
                continue;
            }
            let marge = v.vente - a.achat;
            let conteneur = match (a.t.max_conteneur, v.t.max_conteneur) {
                (x, y) if x > 0.0 && y > 0.0 => x.min(y),
                (x, y) => x.max(y),
            };
            routes.push(Route {
                marchandise: a.nom.clone(),
                illegale: illegales.contains(&a.commodite),
                achat: etape(a, a.achat, a.scu_achat),
                vente: etape(v, v.vente, v.scu_vente),
                scu,
                investissement: scu * a.achat,
                benefice: scu * marge,
                marge_unitaire: marge,
                conteneur_max: conteneur,
            });
        }
    }
    routes.sort_by(|x, y| y.benefice.total_cmp(&x.benefice));
    routes.truncate(p.limite.unwrap_or(25).min(100));
    routes
}

#[tauri::command(async)]
pub fn commerce_routes(dossiers: tauri::State<'_, Dossiers>, parametres: Parametres) -> Result<Vec<Route>, String> {
    if !(parametres.scu > 0.0) {
        return Err("Choisis un vaisseau ou indique une soute (SCU).".into());
    }
    let (prix, _) = charger(&dossiers, "commodities_prices_all", TTL_PRIX, false)?;
    let (term, _) = charger(&dossiers, "terminals", TTL_REFERENTIEL, false)?;
    let (com, _) = charger(&dossiers, "commodities", TTL_REFERENTIEL, false)?;
    let illegales: Vec<i64> =
        com.as_array().into_iter().flatten().filter(|c| nombre(&c["is_illegal"]) > 0.0).filter_map(|c| c["id"].as_i64()).collect();
    Ok(calculer(&prix, &terminaux(&term), &illegales, &parametres))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_jeton_uex() {
        assert!(jeton_valide("0123456789abcdef0123456789abcdef01234567"));
        assert!(!jeton_valide("trop-court"));
        assert!(!jeton_valide("0123456789abcdef0123456789abcdef0123456g"));
    }
    use serde_json::json;

    fn donnees() -> (Value, HashMap<i64, Terminal>) {
        let now = maintenant() as i64;
        let term = json!([
          { "id": 1, "type": "commodity", "displayname": "Mine A", "moon_name": "Daymar", "star_system_name": "Stanton", "is_available_live": 1, "max_container_size": 16 },
          { "id": 2, "type": "commodity", "displayname": "Area18", "planet_name": "ArcCorp", "city_name": "Area 18", "star_system_name": "Stanton", "is_available_live": 1, "max_container_size": 32 },
          { "id": 3, "type": "commodity", "displayname": "Ruin", "planet_name": "Pyro I", "star_system_name": "Pyro", "is_available_live": 1 },
          { "id": 4, "type": "commodity", "displayname": "Pyro Gateway", "space_station_name": "Pyro Gateway (Stanton)", "orbit_name": "Pyro Gateway (Stanton system)", "star_system_name": "Stanton", "is_available_live": 1 }
        ]);
        let prix = json!([
          { "id_commodity": 10, "id_terminal": 1, "price_buy": 100, "price_sell": 0, "scu_buy": 50, "date_modified": now, "commodity_name": "Agricium" },
          { "id_commodity": 10, "id_terminal": 2, "price_buy": 0, "price_sell": 150, "scu_sell": 1000, "date_modified": now, "commodity_name": "Agricium" },
          { "id_commodity": 10, "id_terminal": 3, "price_buy": 0, "price_sell": 400, "scu_sell": 1000, "date_modified": now - 30 * 86400, "commodity_name": "Agricium" },
          { "id_commodity": 20, "id_terminal": 1, "price_buy": 10, "price_sell": 0, "date_modified": now, "commodity_name": "WiDoW" },
          { "id_commodity": 20, "id_terminal": 2, "price_buy": 0, "price_sell": 90, "date_modified": now, "commodity_name": "WiDoW" }
        ]);
        (prix, terminaux(&term))
    }

    fn p(scu: f64) -> Parametres {
        Parametres { scu, budget: None, systeme: None, depart: None, illegales: None, age_max_jours: None, limite: None }
    }

    #[test]
    fn route_limitee_par_stock_et_prix_trop_vieux_ignore() {
        let (prix, t) = donnees();
        let r = calculer(&prix, &t, &[20], &p(456.0));
        assert_eq!(r.len(), 1, "le prix de 30 jours et l'illégal sont exclus");
        assert_eq!(r[0].scu, 50.0); // stock à l'achat
        assert_eq!(r[0].benefice, 50.0 * 50.0);
        assert_eq!(r[0].achat.lieu, "Daymar");
        assert_eq!(r[0].vente.lieu, "Area 18 · ArcCorp");
        assert_eq!(r[0].conteneur_max, 16.0);
        assert_eq!(t[&4].lieu, "", "le lieu ne répète pas le nom du terminal");
    }

    #[test]
    fn budget_illegales_et_age() {
        let (prix, t) = donnees();
        let mut q = p(456.0);
        q.budget = Some(1000.0);
        assert_eq!(calculer(&prix, &t, &[20], &q)[0].scu, 10.0);
        q.budget = None;
        q.illegales = Some(true);
        assert_eq!(calculer(&prix, &t, &[20], &q)[0].marchandise, "WiDoW"); // 80 × 456 > 50 × 50
        q.age_max_jours = Some(60.0);
        q.illegales = None;
        let r = calculer(&prix, &t, &[20], &q);
        assert_eq!(r[0].vente.systeme, "Pyro");
        q.systeme = Some("Stanton".into());
        assert!(calculer(&prix, &t, &[20], &q).iter().all(|x| x.vente.systeme == "Stanton"));
    }
}
