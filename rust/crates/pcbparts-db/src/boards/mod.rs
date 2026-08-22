//! Boards database: schema + module wiring.
pub mod search;
pub mod detail;

#[cfg(test)]
pub(crate) mod fixtures;

pub const SCHEMA: &str = "
CREATE TABLE boards (
    id INTEGER PRIMARY KEY,
    slug TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    org TEXT,
    org_display TEXT,
    source TEXT,
    format TEXT,
    description TEXT,
    key_coverage TEXT,
    layers INTEGER,
    width_mm REAL,
    height_mm REAL,
    min_trace TEXT,
    min_clearance TEXT,
    min_drill TEXT,
    min_via TEXT,
    component_count INTEGER,
    ic_count INTEGER,
    net_count INTEGER,
    key_ics_text TEXT,
    all_ics_text TEXT,
    nets_json TEXT,
    positions_json TEXT,
    copper_pours_json TEXT,
    neighborhoods_json TEXT
);
CREATE TABLE board_tags (
    board_id INTEGER NOT NULL REFERENCES boards(id),
    tag TEXT NOT NULL,
    PRIMARY KEY (board_id, tag)
);
CREATE TABLE board_key_ics (
    board_id INTEGER NOT NULL REFERENCES boards(id),
    ic TEXT NOT NULL,
    PRIMARY KEY (board_id, ic)
);
CREATE TABLE board_components (
    id INTEGER PRIMARY KEY,
    board_id INTEGER NOT NULL REFERENCES boards(id),
    ref TEXT NOT NULL,
    value TEXT,
    footprint TEXT,
    description TEXT,
    voltage TEXT,
    tolerance TEXT,
    dielectric TEXT,
    decouples TEXT,
    pullup TEXT,
    pulldown TEXT
);
CREATE INDEX idx_board_org ON boards(org);
CREATE INDEX idx_board_layers ON boards(layers);
CREATE INDEX idx_board_format ON boards(format);
CREATE INDEX idx_board_component_count ON boards(component_count DESC);
CREATE INDEX idx_board_tag ON board_tags(tag);
CREATE INDEX idx_board_key_ic ON board_key_ics(ic);
CREATE INDEX idx_comp_board_id ON board_components(board_id);
CREATE INDEX idx_comp_value ON board_components(value);
CREATE VIRTUAL TABLE boards_fts USING fts5(
    slug, name, description, key_coverage, tags_text, key_ics_text, all_ics_text, org_display,
    tokenize='porter unicode61'
);
";

use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

/// Opens an existing boards.db file (built by the offline pipeline).
///
/// NOTE: unlike Python's `BoardsDatabase`, this does not build the DB if
/// missing — `pcbparts-pipeline` (a later migration phase) owns the Rust
/// builder. `Mutex<Connection>` gives correct concurrent access for now;
/// the production connection-pooling strategy is decided in the
/// `pcbparts-server` phase once the async runtime is in place.
pub struct BoardsDb {
    conn: Mutex<Connection>,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("boards database not found at {0}")]
    NotFound(String),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

impl BoardsDb {
    pub fn open(db_path: &Path) -> Result<Self, OpenError> {
        if !db_path.exists() {
            return Err(OpenError::NotFound(db_path.display().to_string()));
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL")?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn search(
        &self,
        query: Option<&str>,
        component: Option<&str>,
        tag: Option<&[&str]>,
        org: Option<&str>,
        layers: Option<i64>,
        limit: i64,
    ) -> search::SearchBoardsResult {
        let conn = self.conn.lock().unwrap();
        search::search_boards(&conn, query, component, tag, org, layers, limit)
    }

    pub fn get_board(
        &self,
        slug: &str,
        include_raw: bool,
        include_bom: bool,
        focus: Option<&str>,
    ) -> Option<serde_json::Value> {
        let conn = self.conn.lock().unwrap();
        detail::get_board(&conn, slug, include_raw, include_bom, focus)
    }

    pub fn get_consensus(&self, ic_name: &str) -> Option<serde_json::Value> {
        let conn = self.conn.lock().unwrap();
        detail::get_consensus(&conn, ic_name)
    }

    pub fn get_tag_consensus(&self, tag: &str) -> Option<serde_json::Value> {
        let conn = self.conn.lock().unwrap();
        detail::get_tag_consensus(&conn, tag)
    }

    pub fn get_stats(&self) -> serde_json::Value {
        let conn = self.conn.lock().unwrap();
        search::get_stats(&conn)
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Writes a real on-disk boards.db (schema + one row) and confirms
    /// `BoardsDb::open` reads it back — proves the production open path
    /// (as opposed to the in-memory fixtures the unit tests use).
    #[test]
    fn opens_real_file_and_searches() {
        let dir = std::env::temp_dir().join(format!("pcbparts-boards-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("boards.db");

        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(SCHEMA).unwrap();
            // format/org/org_display/description/key_coverage/*_text are never NULL in
            // real builder output (build_boards_db.py defaults every field with `.get(k, "")`),
            // so a realistic row sets them all — mirrors get_stats()'s reliance on that invariant.
            conn.execute(
                "INSERT INTO boards (slug, name, org, org_display, source, format, description, \
                 key_coverage, key_ics_text, all_ics_text, component_count, ic_count, net_count) \
                 VALUES ('x', 'X Board', 'acme', 'Acme', 'acme/x', 'kicad7', '', '', '', '', 1, 0, 0)",
                [],
            ).unwrap();
        }

        let db = BoardsDb::open(&db_path).unwrap();
        let stats = db.get_stats();
        assert_eq!(stats["total_boards"], 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_errors_clearly() {
        let missing = Path::new("/nonexistent/path/boards.db");
        assert!(matches!(BoardsDb::open(missing), Err(OpenError::NotFound(_))));
    }
}
