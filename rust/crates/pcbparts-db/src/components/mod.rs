//! Component database: schema doc-reference + module wiring.
//!
//! Merges Python's `connection.py` (DB open + cache loading) and `db/__init__.py`
//! (the `ComponentDatabase` facade) into one module — in Python, `connection.py`
//! exists solely to be called from `__init__.py`'s `_ensure_db()`; there is no
//! independent Rust caller for it.
pub mod attributes;
pub mod categories;
pub mod lookup;
pub mod stats;

#[cfg(test)]
pub(crate) mod fixtures;

use pcbparts_search::engine::{CategoryInfo, SearchEngine, SearchParams};
use pcbparts_search::result::SubcategoryInfo;
use rusqlite::Connection;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Mutex;

/// Matches Python's `config.py::DEFAULT_MIN_STOCK`. Declared locally rather than
/// depending on `pcbparts-server` (Phase 9) for one integer — see this plan's header
/// and the spec's Phase 5 corrections.
pub const DEFAULT_MIN_STOCK: i64 = 10;

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("components database not found at {0}")]
    NotFound(String),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

/// Opens an existing components.db file (built by the offline pipeline).
///
/// NOTE: unlike Python's `ComponentDatabase._ensure_db()`, this does not build the
/// DB if missing — `pcbparts-pipeline` (a later migration phase) owns the Rust
/// builder. See `boards::BoardsDb`/`sensor::SensorDb` doc comments for the same
/// reasoning; this crate has established the pattern already.
pub struct ComponentsDb {
    conn: Mutex<Connection>,
    subcategories: BTreeMap<i64, SubcategoryInfo>,
    categories: BTreeMap<i64, CategoryInfo>,
    subcategory_name_to_id: HashMap<String, i64>,
    category_name_to_id: HashMap<String, i64>,
    category_to_subcategories: BTreeMap<i64, Vec<i64>>,
    search_engine: SearchEngine,
}

impl ComponentsDb {
    pub fn open(db_path: &Path) -> Result<Self, OpenError> {
        if !db_path.exists() {
            return Err(OpenError::NotFound(db_path.display().to_string()));
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL")?;

        let mut subcategories: BTreeMap<i64, SubcategoryInfo> = BTreeMap::new();
        let mut subcategory_name_to_id: HashMap<String, i64> = HashMap::new();
        let mut category_to_subcategories: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
        {
            let mut stmt = conn.prepare("SELECT id, category_id, name, category_name FROM subcategories")?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let id: i64 = row.get(0)?;
                let category_id: i64 = row.get(1)?;
                let name: String = row.get(2)?;
                let category_name: Option<String> = row.get(3)?;
                subcategory_name_to_id.insert(name.to_lowercase(), id);
                category_to_subcategories.entry(category_id).or_default().push(id);
                subcategories.insert(id, SubcategoryInfo { name, category_id, category_name });
            }
        }

        let mut categories: BTreeMap<i64, CategoryInfo> = BTreeMap::new();
        let mut category_name_to_id: HashMap<String, i64> = HashMap::new();
        {
            let mut stmt = conn.prepare("SELECT id, name FROM categories")?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let id: i64 = row.get(0)?;
                let name: String = row.get(1)?;
                category_name_to_id.insert(name.to_lowercase(), id);
                categories.insert(id, CategoryInfo { name });
            }
        }

        let search_engine = SearchEngine::new(
            subcategories.clone(),
            categories.clone(),
            subcategory_name_to_id.clone(),
            category_name_to_id.clone(),
            category_to_subcategories.clone(),
        );

        Ok(Self {
            conn: Mutex::new(conn),
            subcategories,
            categories,
            subcategory_name_to_id,
            category_name_to_id,
            category_to_subcategories,
            search_engine,
        })
    }

    pub fn resolve_subcategory_name(&self, name: &str) -> Option<i64> {
        self.search_engine.resolve_subcategory_name(name)
    }

    pub fn resolve_category_name(&self, name: &str) -> Option<i64> {
        self.search_engine.resolve_category_name(name)
    }

    pub fn subcategory_display_name(&self, subcategory_id: i64) -> Option<&str> {
        self.subcategories.get(&subcategory_id).map(|i| i.name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::fixtures::real_db_path;

    fn real_db() -> ComponentsDb {
        ComponentsDb::open(&real_db_path()).expect(
            "data/components.db must exist and be built — see this plan's Global Constraints",
        )
    }

    #[test]
    fn missing_file_errors_clearly() {
        let missing = std::path::Path::new("/nonexistent/path/components.db");
        assert!(matches!(ComponentsDb::open(missing), Err(OpenError::NotFound(_))));
    }

    // --- TestNameResolution ---
    #[test]
    fn test_resolve_subcategory_name_exact() {
        let db = real_db();
        let result = db.resolve_subcategory_name("MOSFETs");
        assert!(result.is_some());
    }

    #[test]
    fn test_resolve_subcategory_name_case_insensitive() {
        let db = real_db();
        let r1 = db.resolve_subcategory_name("mosfets");
        let r2 = db.resolve_subcategory_name("MOSFETS");
        let r3 = db.resolve_subcategory_name("MoSfEtS");
        assert_eq!(r1, r2);
        assert_eq!(r2, r3);
    }

    #[test]
    fn test_resolve_subcategory_name_partial_match() {
        let db = real_db();
        assert!(db.resolve_subcategory_name("Chip Resistor").is_some());
    }

    #[test]
    fn test_resolve_subcategory_name_not_found() {
        let db = real_db();
        assert_eq!(db.resolve_subcategory_name("NonExistentCategory12345"), None);
    }

    #[test]
    fn test_resolve_category_name_exact() {
        let db = real_db();
        assert!(db.resolve_category_name("Resistors").is_some());
    }

    #[test]
    fn test_resolve_category_name_case_insensitive() {
        let db = real_db();
        let r1 = db.resolve_category_name("capacitors");
        let r2 = db.resolve_category_name("CAPACITORS");
        assert_eq!(r1, r2);
    }

    // --- TestSubcategoryAliases ---
    #[test]
    fn test_mlcc_alias() {
        let db = real_db();
        let id = db.resolve_subcategory_name("MLCC").unwrap();
        let name = db.subcategory_display_name(id).unwrap();
        assert!(name.contains("SMD") || name.to_lowercase().contains("smd"));
        assert!(!name.contains("Leaded"));
    }

    #[test]
    fn test_common_aliases() {
        let db = real_db();
        let cases = [("mosfet", "MOSFETs"), ("schottky", "Schottky Diodes"), ("crystal", "Crystals")];
        for (alias, expected) in cases {
            let id = db.resolve_subcategory_name(alias).unwrap_or_else(|| panic!("alias '{alias}' should resolve"));
            let name = db.subcategory_display_name(id).unwrap();
            assert_eq!(name, expected, "'{alias}' should resolve to '{expected}', got '{name}'");
        }
    }

    #[test]
    fn test_esd_alias_resolves_to_tvs_esd() {
        let db = real_db();
        for alias in ["ESD", "TVS", "esd protection", "surge protection"] {
            let id = db.resolve_subcategory_name(alias).unwrap_or_else(|| panic!("alias '{alias}' should resolve"));
            let name = db.subcategory_display_name(id).unwrap();
            assert!(name.contains("TVS/ESD"), "'{alias}' should resolve to TVS/ESD subcategory, got '{name}'");
        }
    }

    #[test]
    fn test_antenna_aliases() {
        let db = real_db();
        let cases = [
            ("antenna", "antennas"), ("ceramic antenna", "antennas"),
            ("wifi antenna", "antennas"), ("ble antenna", "antennas"),
        ];
        for (alias, expected_lower) in cases {
            let id = db.resolve_subcategory_name(alias).unwrap_or_else(|| panic!("alias '{alias}' should resolve"));
            let name = db.subcategory_display_name(id).unwrap();
            assert_eq!(name.to_lowercase(), expected_lower, "'{alias}' should resolve to '{expected_lower}', got '{name}'");
        }
    }

    #[test]
    fn test_temperature_humidity_sensor_word_order() {
        let db = real_db();
        let expected = "temperature and humidity sensor";
        for alias in ["humidity temperature sensor", "temperature humidity sensor", "temp humidity sensor", "humidity temp sensor"] {
            let id = db.resolve_subcategory_name(alias).unwrap_or_else(|| panic!("alias '{alias}' should resolve"));
            let name = db.subcategory_display_name(id).unwrap();
            assert_eq!(name.to_lowercase(), expected, "'{alias}' should resolve to '{expected}', got '{name}'");
        }
    }

    // --- TestShortestMatchPriority ---
    #[test]
    fn test_crystal_resolves_to_crystals() {
        let db = real_db();
        let id = db.resolve_subcategory_name("crystal").unwrap();
        assert_eq!(db.subcategory_display_name(id).unwrap(), "Crystals");
    }

    // test_search_shows_resolved_name (the other TestShortestMatchPriority test) is
    // ported in Task 6 — it asserts through `search()`, not name resolution alone.

    // --- TestExtendedAliases ---
    #[test]
    fn test_dc_dc_aliases() {
        let db = real_db();
        for alias in ["dc-dc", "dc dc", "buck converter", "boost converter"] {
            assert!(db.resolve_subcategory_name(alias).is_some(), "alias '{alias}' should resolve");
        }
    }

    #[test]
    fn test_sensor_aliases() {
        let db = real_db();
        for alias in ["hall sensor", "temperature sensor", "current sensor"] {
            assert!(db.resolve_subcategory_name(alias).is_some(), "alias '{alias}' should resolve");
        }
    }

    #[test]
    fn test_module_aliases() {
        let db = real_db();
        for alias in ["wifi module", "bluetooth module", "lora module"] {
            assert!(db.resolve_subcategory_name(alias).is_some(), "alias '{alias}' should resolve");
        }
    }
}
