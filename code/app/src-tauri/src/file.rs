//! File locale des événements à envoyer au portail (SQLite dans `%APPDATA%\fr.ces-sc.compagnon\`).
//!
//! Survit à une coupure réseau, à un redémarrage du PC ou de l'application : les événements partent dès que le
//! portail répond. L'`id` de chaque événement est stable (dérivé de sa ligne du journal) : `INSERT OR IGNORE`
//! ici, et le portail dédoublonne aussi de son côté.

use rusqlite::{params, Connection};
use serde_json::Value;
use std::path::Path;
use std::sync::Mutex;

pub struct File {
    conn: Mutex<Connection>,
}

impl File {
    pub fn ouvrir(chemin: &Path) -> rusqlite::Result<File> {
        if let Some(d) = chemin.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let conn = Connection::open(chemin)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS evenements (
               id TEXT PRIMARY KEY,
               type TEXT NOT NULL,
               at TEXT NOT NULL,
               operation_id TEXT,
               data TEXT NOT NULL,
               ajoute TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS evenements_at ON evenements(at);",
        )?;
        Ok(File { conn: Mutex::new(conn) })
    }

    pub fn ajouter(&self, id: &str, type_: &str, at: &str, operation_id: Option<&str>, data: &Value, ajoute: &str) {
        let c = self.conn.lock().unwrap();
        let _ = c.execute(
            "INSERT OR IGNORE INTO evenements (id, type, at, operation_id, data, ajoute) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, type_, at, operation_id, data.to_string(), ajoute],
        );
    }

    /// Les `n` plus anciens, au format du contrat (`{ id, type, at, operationId, data }`).
    pub fn lot(&self, n: usize) -> Vec<Value> {
        let c = self.conn.lock().unwrap();
        let Ok(mut req) = c.prepare("SELECT id, type, at, operation_id, data FROM evenements ORDER BY at LIMIT ?1") else {
            return Vec::new();
        };
        let lignes = req.query_map(params![n as i64], |r| {
            let data: String = r.get(4)?;
            Ok(serde_json::json!({
                "id": r.get::<_, String>(0)?,
                "type": r.get::<_, String>(1)?,
                "at": r.get::<_, String>(2)?,
                "operationId": r.get::<_, Option<String>>(3)?,
                "data": serde_json::from_str::<Value>(&data).unwrap_or(Value::Null),
            }))
        });
        lignes.map(|it| it.filter_map(Result::ok).collect()).unwrap_or_default()
    }

    pub fn retirer(&self, ids: &[String]) {
        let c = self.conn.lock().unwrap();
        for id in ids {
            let _ = c.execute("DELETE FROM evenements WHERE id = ?1", params![id]);
        }
    }

    /// Abandonne ce que le portail refuserait de toute façon (plus de 14 jours).
    pub fn purger_avant(&self, limite_iso: &str) {
        let c = self.conn.lock().unwrap();
        let _ = c.execute("DELETE FROM evenements WHERE at < ?1", params![limite_iso]);
    }

    /// Vide la file (appareil délié : ces événements n'ont plus de destinataire).
    pub fn vider(&self) {
        let c = self.conn.lock().unwrap();
        let _ = c.execute("DELETE FROM evenements", []);
    }

    pub fn taille(&self) -> u64 {
        let c = self.conn.lock().unwrap();
        c.query_row("SELECT COUNT(*) FROM evenements", [], |r| r.get::<_, i64>(0)).map(|n| n as u64).unwrap_or(0)
    }
}
