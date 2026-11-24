//! OCR À LA DEMANDE de l'écran de raffinerie (feuille de route § 4 ; PROMPT-PC § 2 : « capture d'écran + OCR à la
//! demande »). Jamais en continu : une seule capture, déclenchée par un clic du membre, après un compte à rebours.
//!
//! - Capture : GDI (copie de l'écran principal, comme l'outil Capture de Windows) — rien ne touche au jeu.
//! - OCR : `Windows.Media.Ocr`, intégré à Windows (langues du profil de l'utilisateur).
//! - L'image reste en mémoire et est oubliée aussitôt : jamais enregistrée, jamais envoyée.
//! - Le résultat n'est qu'une PROPOSITION : le membre la corrige avant de l'ajouter à sa session.

use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Detection {
    pub nom: String,
    pub scu: f64,
}

#[derive(Serialize)]
pub struct Lecture {
    pub lignes: Vec<String>,
    pub detections: Vec<Detection>,
}

fn sans_accents(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            _ => c,
        })
        .collect()
}

/// Nom de base d'un minerai pour la recherche (« Quantainium (Raw) » → « quantainium »).
fn base(nom: &str) -> String {
    let n = sans_accents(&nom.to_lowercase());
    n.split('(').next().unwrap_or(&n).trim().to_string()
}

/// Premier nombre d'une ligne, en SCU (« 12,5 SCU », « 1 250 cSCU » → 12,5 ; 12,5).
fn quantite(ligne: &str) -> Option<f64> {
    let l = ligne.replace('\u{a0}', " ");
    let mut nombre = String::new();
    let mut fin = 0;
    let octets: Vec<char> = l.chars().collect();
    let mut i = 0;
    while i < octets.len() {
        let c = octets[i];
        if c.is_ascii_digit() || (!nombre.is_empty() && (c == ',' || c == '.') && octets.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            nombre.push(if c == ',' { '.' } else { c });
            fin = i + 1;
        } else if !nombre.is_empty() && c == ' ' && octets.get(i + 1).is_some_and(|d| d.is_ascii_digit()) && nombre.len() <= 3 && !nombre.contains('.') {
            // séparateur de milliers « 1 250 »
        } else if !nombre.is_empty() {
            break;
        }
        i += 1;
    }
    let mut v: f64 = nombre.parse().ok()?;
    let suite: String = octets[fin..].iter().collect::<String>().trim_start().to_lowercase();
    if suite.starts_with("cscu") {
        v /= 100.0;
    }
    Some(v)
}

/// Repère les minerais connus et leur quantité dans les lignes reconnues (pur, testable).
pub fn analyser(lignes: &[String], minerais: &[String]) -> Vec<Detection> {
    let mut connus: Vec<(String, &String)> = minerais.iter().map(|m| (base(m), m)).filter(|(b, _)| b.len() >= 3).collect();
    connus.sort_by(|a, b| b.0.len().cmp(&a.0.len())); // le plus long d'abord (« laranite » avant « lara… »)
    let mut sortie: Vec<Detection> = Vec::new();
    for (i, ligne) in lignes.iter().enumerate() {
        let l = sans_accents(&ligne.to_lowercase());
        let Some((_, nom)) = connus.iter().find(|(b, _)| l.contains(b.as_str())) else { continue };
        if sortie.iter().any(|d| &d.nom == *nom) {
            continue;
        }
        // quantité sur la même ligne (après le nom), sinon sur la ligne suivante
        let apres = l.split_once(base(nom).as_str()).map(|(_, a)| a.to_string()).unwrap_or_default();
        let q = quantite(&apres).or_else(|| lignes.get(i + 1).and_then(|s| quantite(s)));
        if let Some(scu) = q.filter(|q| *q > 0.0 && *q < 100_000.0) {
            sortie.push(Detection { nom: (*nom).clone(), scu });
        }
    }
    sortie
}

// --- Capture et OCR (Windows) ------------------------------------------------------------------------------

#[cfg(windows)]
fn capturer() -> Result<(Vec<u8>, i32, i32), String> {
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    unsafe {
        let (w, h) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
        if w <= 0 || h <= 0 {
            return Err("Écran introuvable.".into());
        }
        let ecran = GetDC(None);
        let memoire = CreateCompatibleDC(Some(ecran));
        let image = CreateCompatibleBitmap(ecran, w, h);
        let ancien = SelectObject(memoire, image.into());
        let copie = BitBlt(memoire, 0, 0, w, h, Some(ecran), 0, 0, SRCCOPY);
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // de haut en bas
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let lues = GetDIBits(memoire, image, 0, h as u32, Some(pixels.as_mut_ptr() as *mut _), &mut info, DIB_RGB_COLORS);
        SelectObject(memoire, ancien);
        let _ = DeleteObject(image.into());
        let _ = DeleteDC(memoire);
        ReleaseDC(None, ecran);
        if copie.is_err() || lues == 0 {
            return Err("Capture d'écran impossible.".into());
        }
        Ok((pixels, w, h))
    }
}

#[cfg(windows)]
fn reconnaitre(pixels: Vec<u8>, w: i32, h: i32) -> Result<Vec<String>, String> {
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Security::Cryptography::CryptographicBuffer;
    let e = |x: windows::core::Error| format!("OCR Windows indisponible : {}", x.message());
    let tampon = CryptographicBuffer::CreateFromByteArray(&pixels).map_err(e)?;
    drop(pixels);
    let image = SoftwareBitmap::CreateCopyFromBuffer(&tampon, BitmapPixelFormat::Bgra8, w, h).map_err(e)?;
    let moteur = OcrEngine::TryCreateFromUserProfileLanguages().map_err(e)?;
    let resultat = moteur.RecognizeAsync(&image).map_err(e)?.join().map_err(e)?;
    let lignes = resultat.Lines().map_err(e)?;
    let mut v = Vec::new();
    for l in lignes {
        if let Ok(t) = l.Text() {
            v.push(t.to_string());
        }
    }
    Ok(v)
}

/// Compte à rebours (pour revenir au jeu), UNE capture, OCR, puis analyse. L'image n'est jamais conservée.
#[tauri::command(async)]
pub fn ocr_raffinerie(delai: Option<u64>, minerais: Vec<String>) -> Result<Lecture, String> {
    std::thread::sleep(std::time::Duration::from_secs(delai.unwrap_or(5).min(15)));
    #[cfg(windows)]
    {
        let (pixels, w, h) = capturer()?;
        let lignes = reconnaitre(pixels, w, h)?;
        let detections = analyser(&lignes, &minerais);
        Ok(Lecture { lignes, detections })
    }
    #[cfg(not(windows))]
    {
        let _ = minerais;
        Err("Disponible seulement sous Windows.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn quantites() {
        assert_eq!(quantite(" 12,5 SCU"), Some(12.5));
        assert_eq!(quantite("1 250 cSCU"), Some(12.5));
        assert_eq!(quantite(": 36"), Some(36.0));
        assert_eq!(quantite("aucun"), None);
    }

    #[test]
    fn ecran_de_raffinerie_simule() {
        let minerais = s(&["Quantainium (Raw)", "Laranite (Ore)", "Inert Materials", "Agricium (Ore)"]);
        let lignes = s(&[
            "MATÉRIAUX À RAFFINER",
            "QUANTAINIUM (RAW)   14,3 SCU",
            "Laranite (Ore)",
            "8.75 SCU",
            "INERT MATERIALS 120 cSCU",
            "Méthode : Dinyx Solventation",
        ]);
        assert_eq!(
            analyser(&lignes, &minerais),
            vec![
                Detection { nom: "Quantainium (Raw)".into(), scu: 14.3 },
                Detection { nom: "Laranite (Ore)".into(), scu: 8.75 },
                Detection { nom: "Inert Materials".into(), scu: 1.2 },
            ]
        );
    }
}

#[cfg(all(test, windows))]
mod test_reel {
    /// Capture réelle + OCR Windows. N'affiche que le NOMBRE de lignes (jamais leur contenu).
    /// cargo test --lib -- --ignored ocr_reel --nocapture
    #[test]
    #[ignore]
    fn ocr_reel() {
        let (pixels, w, h) = super::capturer().expect("capture");
        let n = super::reconnaitre(pixels, w, h).expect("ocr").len();
        println!("écran {w}x{h} : {n} lignes reconnues");
        assert!(n > 0);
    }
}
