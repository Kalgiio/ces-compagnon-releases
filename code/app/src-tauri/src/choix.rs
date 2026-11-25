//! Fichiers choisis par le membre dans une boîte de dialogue Windows (audit de sécurité, 25/11).
//!
//! L'interface ne reçoit JAMAIS le chemin d'un fichier : seulement un jeton opaque (`choix-<uuid>`), que le cœur Rust
//! retraduit en chemin au moment de l'envoi. Une interface compromise (contenu web malveillant) ne peut donc pas faire
//! lire ni envoyer un fichier du PC que le membre n'a pas choisi lui-même.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

fn table() -> &'static Mutex<HashMap<String, PathBuf>> {
    static T: OnceLock<Mutex<HashMap<String, PathBuf>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Mémorise un fichier choisi dans une boîte de dialogue et rend son jeton.
pub fn memoriser(chemin: PathBuf) -> String {
    let jeton = format!("choix-{}", uuid::Uuid::new_v4());
    let mut t = table().lock().unwrap();
    if t.len() > 200 {
        t.clear();
    }
    t.insert(jeton.clone(), chemin);
    jeton
}

/// Chemin d'un jeton remis par `memoriser` ; tout autre texte est refusé.
pub fn chemin(jeton: &str) -> Result<PathBuf, String> {
    table().lock().unwrap().get(jeton).cloned().ok_or_else(|| "Choisis de nouveau le fichier.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_chemin_brut_est_refuse() {
        assert!(chemin(r"C:\Windows\win.ini").is_err());
        let j = memoriser(PathBuf::from(r"C:\photo.png"));
        assert_eq!(chemin(&j).unwrap(), PathBuf::from(r"C:\photo.png"));
    }
}
