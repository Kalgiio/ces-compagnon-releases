//! Fond d'écran personnel (demande du CES, 21/11) : le membre choisit sa propre image, gardée sur ce PC seulement.
//! L'image est relue, réduite à 2560 px de large au plus et réenregistrée en JPEG (`fond-perso.jpg` des données) :
//! le fichier d'origine n'est jamais servi tel quel, et l'interface la reçoit en `data:` URL.

use crate::traduction::Dossiers;
use base64::Engine;
use std::path::PathBuf;

const MAX_OCTETS: u64 = 25 * 1024 * 1024;
const LARGEUR_MAX: u32 = 2560;

fn fichier(d: &Dossiers) -> PathBuf {
    d.donnees.join("fond-perso.jpg")
}

/// Relit, réduit et encode en JPEG (qualité 86).
fn preparer(octets: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(octets).map_err(|_| "Ce fichier n'est pas une image lisible (JPEG, PNG ou WebP).".to_string())?;
    let img = if img.width() > LARGEUR_MAX { img.resize(LARGEUR_MAX, u32::MAX, image::imageops::FilterType::Triangle) } else { img };
    let mut sortie = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut sortie, 86);
    img.to_rgb8().write_with_encoder(enc).map_err(|e| e.to_string())?;
    Ok(sortie)
}

fn data_url(octets: &[u8]) -> String {
    format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(octets))
}

/// Fond personnel enregistré, en `data:` URL, ou `None`.
#[tauri::command(async)]
pub fn fond_perso_lire(dossiers: tauri::State<'_, Dossiers>) -> Option<String> {
    std::fs::read(fichier(&dossiers)).ok().map(|o| data_url(&o))
}

/// Choisir une image (boîte de dialogue Windows) → enregistrée et renvoyée en `data:` URL ; `None` si annulé.
#[tauri::command(async)]
pub fn fond_perso_choisir(app: tauri::AppHandle, dossiers: tauri::State<'_, Dossiers>) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let Some(choix) = app.dialog().file().set_title("Image de fond").add_filter("Images", &["jpg", "jpeg", "png", "webp"]).blocking_pick_file() else {
        return Ok(None);
    };
    let p = choix.into_path().map_err(|e| e.to_string())?;
    if std::fs::metadata(&p).map(|m| m.len()).unwrap_or(u64::MAX) > MAX_OCTETS {
        return Err("Image trop lourde (25 Mo au plus).".into());
    }
    let octets = std::fs::read(&p).map_err(|e| e.to_string())?;
    let jpeg = preparer(&octets)?;
    crate::traduction::ecrire_sur(&fichier(&dossiers), &jpeg).map_err(|e| e.to_string())?;
    Ok(Some(data_url(&jpeg)))
}

#[tauri::command]
pub fn fond_perso_effacer(dossiers: tauri::State<'_, Dossiers>) {
    let _ = std::fs::remove_file(fichier(&dossiers));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduit_et_reencode() {
        let img = image::RgbImage::from_pixel(3000, 1000, image::Rgb([10, 20, 200]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let jpeg = preparer(&png).unwrap();
        let relu = image::load_from_memory(&jpeg).unwrap();
        assert_eq!((relu.width(), relu.height()), (2560, 853));
        assert!(preparer(b"pas une image").is_err());
        assert!(data_url(&jpeg).starts_with("data:image/jpeg;base64,/9j/"));
    }
}
