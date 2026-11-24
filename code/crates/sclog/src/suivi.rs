//! Suivi en direct de `Game.log` : on relit seulement ce qui a été ajouté depuis la dernière fois.
//!
//! Le jeu écrit en continu ; au lancement suivant, il RENOMME l'ancien journal dans `logbackups\` et repart
//! d'un fichier vide. Une taille plus petite que la position connue, ou un début de fichier différent,
//! signale ce nouveau fichier : on repart de zéro.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Taille lue au plus par appel : garde la boucle réactive au premier passage sur un journal de 100 Mo.
const BLOC_MAX: usize = 8 << 20;
const ENTETE: usize = 128;

pub struct Suiveur {
    chemin: PathBuf,
    position: u64,
    entete: Vec<u8>,
    reste: Vec<u8>,
}

/// Résultat d'une lecture.
#[derive(Debug, Default)]
pub struct Lecture {
    /// Nouveau fichier détecté (rotation) : l'analyse doit repartir de zéro AVANT de traiter `lignes`.
    pub nouveau_fichier: bool,
    /// Lignes complètes, avec la position (octet) de leur début dans le fichier.
    pub lignes: Vec<(u64, String)>,
    /// Il reste des octets à lire (rappeler tout de suite).
    pub suite: bool,
}

impl Suiveur {
    pub fn nouveau(chemin: impl Into<PathBuf>) -> Self {
        Suiveur { chemin: chemin.into(), position: 0, entete: Vec::new(), reste: Vec::new() }
    }

    pub fn chemin(&self) -> &Path {
        &self.chemin
    }

    /// Lit ce qui a été ajouté. Fichier absent → lecture vide (pas une erreur : le jeu n'a pas encore écrit).
    pub fn lire(&mut self) -> io::Result<Lecture> {
        let mut f = match File::open(&self.chemin) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Lecture::default()),
            Err(e) => return Err(e),
        };
        let taille = f.metadata()?.len();
        let mut entete = vec![0u8; ENTETE.min(taille as usize)];
        f.read_exact(&mut entete)?;

        let mut lecture = Lecture::default();
        // Même fichier = même début (sur la longueur commune : un fichier tout neuf peut être encore court).
        let n = self.entete.len().min(entete.len());
        let meme_fichier = entete[..n] == self.entete[..n];
        if self.position > 0 && (taille < self.position || !meme_fichier) {
            self.position = 0;
            self.reste.clear();
            lecture.nouveau_fichier = true;
        }
        if lecture.nouveau_fichier || entete.len() > self.entete.len() {
            self.entete = entete;
        }
        if taille == self.position {
            return Ok(lecture);
        }

        let a_lire = ((taille - self.position) as usize).min(BLOC_MAX);
        f.seek(SeekFrom::Start(self.position))?;
        let mut bloc = vec![0u8; a_lire];
        f.read_exact(&mut bloc)?;
        let debut_bloc = self.position - self.reste.len() as u64;
        self.position += a_lire as u64;
        lecture.suite = self.position < taille;

        let mut donnees = std::mem::take(&mut self.reste);
        donnees.extend_from_slice(&bloc);
        let mut depart = 0usize;
        while let Some(i) = donnees[depart..].iter().position(|&b| b == b'\n') {
            let fin = depart + i;
            let brut = &donnees[depart..fin];
            let brut = brut.strip_suffix(b"\r").unwrap_or(brut);
            if !brut.is_empty() {
                lecture.lignes.push((debut_bloc + depart as u64, String::from_utf8_lossy(brut).into_owned()));
            }
            depart = fin + 1;
        }
        self.reste = donnees[depart..].to_vec(); // ligne pas encore terminée : on l'attend
        Ok(lecture)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sclog-test-{}-{}", nom, std::process::id()));
        let _ = std::fs::remove_file(&d);
        d
    }

    #[test]
    fn lit_la_suite_et_attend_les_lignes_incompletes() {
        let p = tmp("suite");
        std::fs::write(&p, "<t1> un\n<t2> de").unwrap();
        let mut s = Suiveur::nouveau(&p);
        let l = s.lire().unwrap();
        assert_eq!(l.lignes, vec![(0, "<t1> un".to_string())]);
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        f.write_all(b"ux\r\n<t3> trois\n").unwrap();
        let l = s.lire().unwrap();
        assert!(!l.nouveau_fichier);
        assert_eq!(l.lignes, vec![(8, "<t2> deux".to_string()), (19, "<t3> trois".to_string())]);
        assert!(s.lire().unwrap().lignes.is_empty());
    }

    #[test]
    fn detecte_la_rotation() {
        let p = tmp("rotation");
        std::fs::write(&p, "<2026-01-01T00:00:00Z> Log started A\n<x> ligne longue\n").unwrap();
        let mut s = Suiveur::nouveau(&p);
        assert_eq!(s.lire().unwrap().lignes.len(), 2);
        std::fs::write(&p, "<2026-01-02T00:00:00Z> Log started B\n").unwrap();
        let l = s.lire().unwrap();
        assert!(l.nouveau_fichier);
        assert_eq!(l.lignes, vec![(0, "<2026-01-02T00:00:00Z> Log started B".to_string())]);
    }

    #[test]
    fn fichier_absent_n_est_pas_une_erreur() {
        let mut s = Suiveur::nouveau(tmp("absent"));
        assert!(s.lire().unwrap().lignes.is_empty());
    }
}
