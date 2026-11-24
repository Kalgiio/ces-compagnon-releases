//! Wikelo (feuille de route § 5) : contrats de l'Emporium de Wikelo et suivi personnel de ce qu'on a réuni.
//!
//! - Source : API du Star Citizen Wiki (`api.star-citizen.wiki`, `GET /api/missions?filter[query]=Wikelo`), sans
//!   clé. Conditions : « Please credit api.star-citizen.wiki in public projects », pas d'usage commercial.
//!   Cache disque de 12 h (les contrats changent avec les patchs).
//! - Suivi : favoris et quantités réunies par objet, dans un fichier local (rien ne part au portail).

use crate::portail;
use crate::traduction::{ecrire_sur, Dossiers};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const URL: &str = "https://api.star-citizen.wiki/api/missions?filter%5Bquery%5D=Wikelo&page%5Bsize%5D=200";
const TTL: u64 = 12 * 3600;

fn maintenant() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Serialize, Deserialize)]
struct Cache {
    recu: u64,
    data: Value,
}

fn charger(d: &Dossiers, forcer: bool) -> Result<(Value, u64), String> {
    let fichier = d.donnees.join("cache").join("wiki-wikelo.json");
    let ancien: Option<Cache> = std::fs::read(&fichier).ok().and_then(|b| serde_json::from_slice(&b).ok());
    if let Some(c) = &ancien {
        if !forcer && maintenant().saturating_sub(c.recu) < TTL {
            return Ok((c.data.clone(), c.recu));
        }
    }
    let r = portail::client()
        .get(URL)
        .header("Accept", "application/json")
        .timeout(Duration::from_secs(60))
        .send()
        .map_err(|e| e.to_string())
        .and_then(|r| r.json::<Value>().map_err(|e| e.to_string()));
    match r {
        Ok(v) if v["data"].is_array() => {
            let c = Cache { recu: maintenant(), data: v["data"].clone() };
            let _ = ecrire_sur(&fichier, &serde_json::to_vec(&c).unwrap_or_default());
            Ok((c.data, c.recu))
        }
        resultat => match ancien {
            Some(c) => Ok((c.data, c.recu)), // wiki indisponible : on garde le cache, même ancien
            None => Err(match resultat {
                Ok(_) => "Réponse inattendue du wiki.".into(),
                Err(e) => format!("Wiki injoignable : {e}"),
            }),
        },
    }
}

/// Catégorie déduite du nom interne du contrat (`TheCollector_Vehicle_…`, `…_Favours…`).
fn categorie(debug: &str, titre: &str) -> &'static str {
    let d = debug.to_lowercase();
    let t = titre.to_lowercase();
    if d.contains("vehicle") {
        "vaisseaux"
    } else if d.contains("favour") || d.contains("intro") || d.contains("foodorder") || t.contains("favor") {
        "faveurs"
    } else if ["armor", "armour", "suit", "_gg_", "_nov", "_apar", "spikey", "orbarm", "heavycombat"].iter().any(|k| d.contains(k)) {
        "armures"
    } else if ["lmg", "pistol", "rifle", "booma", "orbvolt", "_sb_", "gmni", "weapon"].iter().any(|k| d.contains(k)) || t.contains("weapon") {
        "armes"
    } else {
        "autres"
    }
}

/// ⚠️ Client HTTP bloquant : exécuté dans `spawn_blocking` (en développement, reqwest vérifie qu'on n'est pas sur un
/// fil du moteur asynchrone et panique sinon ; les versions publiées n'ont pas cette vérification).
#[tauri::command]
pub async fn wikelo_liste(dossiers: tauri::State<'_, Dossiers>, forcer: Option<bool>) -> Result<Value, String> {
    let d = Dossiers { donnees: dossiers.donnees.clone() };
    tauri::async_runtime::spawn_blocking(move || liste(&d, forcer.unwrap_or(false))).await.map_err(|e| e.to_string())?
}

fn liste(dossiers: &Dossiers, forcer: bool) -> Result<Value, String> {
    let (data, recu) = charger(dossiers, forcer)?;
    let mut version = None;
    let mut contrats: Vec<Value> = data
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["mission_giver"].as_str().is_some_and(|g| g.eq_ignore_ascii_case("wikelo")))
        .map(|m| {
            if version.is_none() {
                version = m["game_version"].as_str().map(str::to_string);
            }
            let elements: Vec<Value> = m["hauling_summary"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|h| {
                    let nom = h["name"].as_str()?;
                    let q = h["max_amount"].as_f64().or_else(|| h["min_amount"].as_f64()).unwrap_or(1.0);
                    Some(json!({ "nom": nom, "quantite": q }))
                })
                .collect();
            json!({
                "uuid": m["uuid"],
                "titre": m["title"],
                "description": m["description"],
                "categorie": categorie(m["debug_name"].as_str().unwrap_or(""), m["title"].as_str().unwrap_or("")),
                "disponible": m["released"].as_bool() == Some(true) && m["not_for_release"].as_bool() != Some(true),
                "reputation": m["reputation_amount"],
                "elements": elements,
                "lien": m["web_url"],
            })
        })
        .collect();
    contrats.sort_by(|a, b| a["titre"].as_str().cmp(&b["titre"].as_str()));
    Ok(json!({ "contrats": contrats, "version": version, "recu": recu }))
}

// --- Détail d'un contrat : récompenses et objets demandés, avec images et description française ---------------

const API: &str = "https://api.star-citizen.wiki/api";

fn get_json(url: &str) -> Result<Value, String> {
    portail::client()
        .get(url)
        .header("Accept", "application/json")
        .timeout(Duration::from_secs(30))
        .send()
        .map_err(|e| format!("Wiki injoignable : {e}"))?
        .json::<Value>()
        .map(|v| v["data"].clone())
        .map_err(|e| e.to_string())
}

fn uuid_sur(u: &str) -> bool {
    u.len() == 36 && u.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// Fiche d'un objet (cache 30 jours) : nom, image (vignette starcitizen.tools), description française du wiki.
fn objet(dossier: &std::path::Path, uuid: &str) -> Value {
    let f = dossier.join(format!("i-{uuid}.json"));
    if let Some(c) = std::fs::read(&f).ok().and_then(|b| serde_json::from_slice::<Cache>(&b).ok()) {
        if maintenant().saturating_sub(c.recu) < 30 * 86400 {
            return c.data;
        }
    }
    let Ok(i) = get_json(&format!("{API}/items/{uuid}")) else { return Value::Null };
    let image = i["images"].as_array().and_then(|a| a.first()).and_then(|x| x["thumbnail_url"].as_str().or(x["original_url"].as_str())).map(str::to_string);
    let v = json!({
        "nom": i["name"],
        "type": i["type_label"],
        "image": image,
        "description": i["description"]["fr_FR"].as_str().filter(|d| !d.trim().is_empty()),
    });
    let _ = ecrire_sur(&f, &serde_json::to_vec(&Cache { recu: maintenant(), data: v.clone() }).unwrap_or_default());
    v
}

fn detail(dossier: &std::path::Path, uuid: &str) -> Result<Value, String> {
    let f = dossier.join(format!("m-{uuid}.json"));
    if let Some(c) = std::fs::read(&f).ok().and_then(|b| serde_json::from_slice::<Cache>(&b).ok()) {
        if maintenant().saturating_sub(c.recu) < 7 * 86400 {
            return Ok(c.data);
        }
    }
    let m = get_json(&format!("{API}/missions/{uuid}"))?;
    let liste = |cle: &str, max: usize| -> Vec<Value> {
        m[cle]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|x| x["uuid"].as_str().is_some_and(uuid_sur))
            .take(max)
            .map(|x| {
                let o = objet(dossier, x["uuid"].as_str().unwrap_or_default());
                json!({
                    "nom": x["name"],
                    "quantite": x["amount"].as_f64().or_else(|| x["max_amount"].as_f64()).unwrap_or(1.0),
                    "image": o["image"],
                    "description": o["description"],
                    "type": o["type"],
                })
            })
            .collect()
    };
    let v = json!({ "recompenses": liste("reward_items", 6), "elements": liste("hauling_orders", 12) });
    let _ = ecrire_sur(&f, &serde_json::to_vec(&Cache { recu: maintenant(), data: v.clone() }).unwrap_or_default());
    Ok(v)
}

/// Récompenses et objets demandés d'un contrat, à l'ouverture (fiches mises en cache sur le disque).
#[tauri::command]
pub async fn wikelo_detail(dossiers: tauri::State<'_, Dossiers>, uuid: String) -> Result<Value, String> {
    if !uuid_sur(&uuid) {
        return Err("Contrat inconnu.".into());
    }
    let dossier = dossiers.donnees.join("cache").join("wikelo");
    tauri::async_runtime::spawn_blocking(move || {
        let _ = std::fs::create_dir_all(&dossier);
        detail(&dossier, &uuid)
    })
    .await
    .map_err(|e| e.to_string())?
}

// --- Suivi personnel ------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Suivi {
    favoris: BTreeSet<String>,
    /// contrat → objet → quantité réunie
    reunis: BTreeMap<String, BTreeMap<String, f64>>,
}

pub struct SuiviWikelo(pub Mutex<Option<Suivi>>);

fn fichier(d: &Dossiers) -> std::path::PathBuf {
    d.donnees.join("wikelo-suivi.json")
}

fn lire(d: &Dossiers, s: &SuiviWikelo) -> Suivi {
    let mut g = s.0.lock().unwrap();
    if g.is_none() {
        *g = Some(std::fs::read(fichier(d)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default());
    }
    g.clone().unwrap_or_default()
}

fn ecrire(d: &Dossiers, s: &SuiviWikelo, v: Suivi) -> Result<Suivi, String> {
    ecrire_sur(&fichier(d), &serde_json::to_vec_pretty(&v).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    *s.0.lock().unwrap() = Some(v.clone());
    Ok(v)
}

fn id_sur(id: &str) -> Result<(), String> {
    (id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')).then_some(()).ok_or_else(|| "Contrat inconnu.".into())
}

#[tauri::command]
pub fn wikelo_suivi(dossiers: tauri::State<'_, Dossiers>, suivi: tauri::State<'_, SuiviWikelo>) -> Suivi {
    lire(&dossiers, &suivi)
}

#[tauri::command]
pub fn wikelo_favori(
    dossiers: tauri::State<'_, Dossiers>,
    suivi: tauri::State<'_, SuiviWikelo>,
    uuid: String,
    favori: bool,
) -> Result<Suivi, String> {
    id_sur(&uuid)?;
    let mut v = lire(&dossiers, &suivi);
    if favori {
        v.favoris.insert(uuid);
    } else {
        v.favoris.remove(&uuid);
    }
    ecrire(&dossiers, &suivi, v)
}

#[tauri::command]
pub fn wikelo_reunir(
    dossiers: tauri::State<'_, Dossiers>,
    suivi: tauri::State<'_, SuiviWikelo>,
    uuid: String,
    objet: String,
    quantite: f64,
) -> Result<Suivi, String> {
    id_sur(&uuid)?;
    let objet: String = objet.chars().take(120).collect();
    let mut v = lire(&dossiers, &suivi);
    let c = v.reunis.entry(uuid.clone()).or_default();
    if quantite > 0.0 {
        c.insert(objet, quantite.min(100_000.0));
    } else {
        c.remove(&objet);
    }
    if c.is_empty() {
        v.reunis.remove(&uuid);
    }
    ecrire(&dossiers, &suivi, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories() {
        assert_eq!(categorie("TheCollector_Vehicle_Large_Anvil_Asgard", "Asgard Fight Mod"), "vaisseaux");
        assert_eq!(categorie("TheColllector_Vehicle_Ground_ATLS_RedNBlue", ""), "vaisseaux");
        assert_eq!(categorie("TheCollector_Favours_01", ""), "faveurs");
        assert_eq!(categorie("TheCollector_SpikeyArmor", "Armor with Vanduul"), "armures");
        assert_eq!(categorie("TheCollector_GMNI_LMG", "Big Style LMG"), "armes");
        assert_eq!(categorie("shapeless veta", "?"), "autres");
    }
}

// --- Proposer une image pour un objet (demande du CES, 23/11 ; contrat API.md § 5 octies) ---------------------

/// Choisir l'image à proposer (JPEG, PNG ou WebP).
#[tauri::command]
pub fn wikelo_choisir_image(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let f = app.dialog().file().set_title("Image à proposer pour cet objet").add_filter("Images", &["jpg", "jpeg", "png", "webp"]).blocking_pick_file()?;
    f.into_path().ok().map(|p| p.display().to_string())
}

/// Envoie la proposition au portail ; la modération du CES la valide avant qu'elle remplace l'image du wiki.
#[tauri::command(async)]
pub fn wikelo_proposer(portail: tauri::State<'_, crate::presence::Portail>, uuid: String, image: String, note: Option<String>) -> Result<Value, String> {
    if !uuid_sur(&uuid) {
        return Err("Objet inconnu.".into());
    }
    let jeton = crate::presence::jeton_actuel(&portail)?;
    let mut form = reqwest::blocking::multipart::Form::new().part("image", crate::personnages::partie_photo(&image)?);
    if let Some(n) = note.map(|n| n.trim().chars().take(200).collect::<String>()).filter(|n| !n.is_empty()) {
        form = form.text("note", n);
    }
    let r = portail::client()
        .post(format!("{}/api/wikelo/items/{uuid}/proposals", portail::ORIGINE))
        .bearer_auth(&jeton)
        .multipart(form)
        .send()
        .map_err(|e| format!("Portail injoignable : {e}"))?;
    if r.status().as_u16() == 404 {
        return Err("Les propositions d'images ne sont pas encore ouvertes sur le portail.".into());
    }
    if !r.status().is_success() {
        return Err(crate::personnages::erreur_http(r));
    }
    r.json::<Value>().map_err(|e| e.to_string())
}
