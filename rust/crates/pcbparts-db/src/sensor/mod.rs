//! Sensor database: schema + module wiring.
pub mod search;

pub const SCHEMA: &str = "
CREATE TABLE sensors (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    manufacturer TEXT,
    type TEXT,
    voltage TEXT,
    datasheet_url TEXT,
    platform_count INTEGER DEFAULT 0,
    description TEXT,
    source_tier TEXT DEFAULT 'primary',
    sources TEXT
);
CREATE TABLE sensor_measures (
    sensor_id TEXT NOT NULL REFERENCES sensors(id),
    measure TEXT NOT NULL,
    PRIMARY KEY (sensor_id, measure)
);
CREATE TABLE sensor_protocols (
    sensor_id TEXT NOT NULL REFERENCES sensors(id),
    protocol TEXT NOT NULL,
    PRIMARY KEY (sensor_id, protocol)
);
CREATE TABLE sensor_platforms (
    sensor_id TEXT NOT NULL REFERENCES sensors(id),
    platform TEXT NOT NULL,
    PRIMARY KEY (sensor_id, platform)
);
CREATE TABLE sensor_urls (
    sensor_id TEXT NOT NULL REFERENCES sensors(id),
    url TEXT NOT NULL,
    PRIMARY KEY (sensor_id, url)
);
CREATE VIRTUAL TABLE sensors_fts USING fts5(id, name, manufacturer, description);
";

use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

/// Opens an existing sensor.db file (built by the offline pipeline).
/// See `boards::BoardsDb` doc comment for the Mutex/pooling and
/// missing-builder notes — the same reasoning applies here.
pub struct SensorDb {
    conn: Mutex<Connection>,
    ic_aliases: HashMap<String, String>,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("sensor database not found at {0}")]
    NotFound(String),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

impl SensorDb {
    /// `data_dir` mirrors Python's constructor: `ic_aliases.json` is loaded
    /// from `data_dir/sensors/ic_aliases.json` if present, else aliases are empty.
    pub fn open(db_path: &Path, data_dir: &Path) -> Result<Self, OpenError> {
        if !db_path.exists() {
            return Err(OpenError::NotFound(db_path.display().to_string()));
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL")?;

        let aliases_path = data_dir.join("sensors").join("ic_aliases.json");
        let ic_aliases = std::fs::read_to_string(&aliases_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        Ok(Self { conn: Mutex::new(conn), ic_aliases })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn search(
        &self,
        query: Option<&str>,
        measure: Option<search::MeasureFilter>,
        r#type: Option<&str>,
        protocol: Option<&str>,
        platform: Option<&str>,
        limit: i64,
    ) -> search::SearchSensorsResult {
        let conn = self.conn.lock().unwrap();
        search::search_sensors(&conn, query, measure, r#type, protocol, platform, limit, &self.ic_aliases)
    }

    pub fn get_stats(&self) -> serde_json::Value {
        let conn = self.conn.lock().unwrap();
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM sensors", [], |r| r.get(0)).unwrap();

        let mut platforms = serde_json::Map::new();
        {
            let mut stmt = conn.prepare("SELECT platform, COUNT(*) FROM sensor_platforms GROUP BY platform ORDER BY 2 DESC").unwrap();
            let mut rows = stmt.query([]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                let p: String = row.get(0).unwrap();
                let c: i64 = row.get(1).unwrap();
                platforms.insert(p, serde_json::json!(c));
            }
        }

        let mut measures = serde_json::Map::new();
        {
            let mut stmt = conn.prepare("SELECT measure, COUNT(*) FROM sensor_measures GROUP BY measure ORDER BY 2 DESC LIMIT 20").unwrap();
            let mut rows = stmt.query([]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                let m: String = row.get(0).unwrap();
                let c: i64 = row.get(1).unwrap();
                measures.insert(m, serde_json::json!(c));
            }
        }

        serde_json::json!({
            "total_sensors": total,
            "platforms": platforms,
            "top_measures": measures,
        })
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn opens_real_file_with_no_aliases_file() {
        let dir = std::env::temp_dir().join(format!("pcbparts-sensor-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("sensor.db");

        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(SCHEMA).unwrap();
            conn.execute(
                "INSERT INTO sensors (id, name, platform_count) VALUES ('x1', 'X1', 0)",
                [],
            ).unwrap();
        }

        let db = SensorDb::open(&db_path, &dir).unwrap();
        let stats = db.get_stats();
        assert_eq!(stats["total_sensors"], 1);
        assert!(db.ic_aliases.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_errors_clearly() {
        let missing = Path::new("/nonexistent/path/sensor.db");
        let missing_dir = Path::new("/nonexistent/path");
        assert!(matches!(SensorDb::open(missing, missing_dir), Err(OpenError::NotFound(_))));
    }
}
