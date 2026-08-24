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
use serde_json::Value;
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

    pub fn get_by_lcsc(&self, lcsc: &str) -> Option<Value> {
        let conn = self.conn.lock().unwrap();
        lookup::get_by_lcsc(&conn, lcsc, &self.subcategories)
    }

    pub fn get_by_mpn(&self, mpn: &str) -> Vec<Value> {
        let conn = self.conn.lock().unwrap();
        lookup::get_by_mpn(&conn, mpn, &self.subcategories)
    }

    pub fn get_by_lcsc_batch(&self, lcsc_codes: &[String]) -> Result<HashMap<String, Option<Value>>, String> {
        let conn = self.conn.lock().unwrap();
        lookup::get_by_lcsc_batch(&conn, lcsc_codes, &self.subcategories)
    }

    pub fn get_subcategory_name(&self, subcategory_id: i64) -> Option<String> {
        categories::get_subcategory_name(subcategory_id, &self.subcategories)
    }

    pub fn get_category_for_subcategory(&self, subcategory_id: i64) -> Option<(i64, Option<String>)> {
        categories::get_category_for_subcategory(subcategory_id, &self.subcategories)
    }

    pub fn get_categories_for_client(&self) -> Vec<Value> {
        let conn = self.conn.lock().unwrap();
        categories::get_categories_for_client(&conn, &self.subcategories)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn find_by_subcategory(
        &self,
        subcategory_id: i64,
        primary_spec: Option<&str>,
        primary_value: Option<&str>,
        min_stock: i64,
        library_type: Option<&str>,
        prefer_no_fee: bool,
        limit: i64,
    ) -> Vec<Value> {
        let conn = self.conn.lock().unwrap();
        let min_stock = min_stock.max(DEFAULT_MIN_STOCK);
        categories::find_by_subcategory(&conn, &self.subcategories, subcategory_id, primary_spec, primary_value, min_stock, library_type, prefer_no_fee, limit)
    }

    pub fn list_attributes(&self, subcategory_id: Option<i64>, subcategory_name: Option<&str>, sample_size: i64) -> Value {
        let conn = self.conn.lock().unwrap();
        attributes::list_attributes(&conn, &self.subcategories, &self.subcategory_name_to_id, subcategory_id, subcategory_name, sample_size)
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

    // --- TestBatchLookup ---
    #[test]
    fn test_batch_lookup_returns_all_parts() {
        let db = real_db();
        let codes = vec!["C1525".to_string(), "C25804".to_string(), "C19702".to_string()];
        let results = db.get_by_lcsc_batch(&codes).unwrap();
        assert_eq!(results.len(), 3);
        for code in &codes {
            assert!(results.contains_key(code));
            assert!(results[code].is_some());
        }
    }

    #[test]
    fn test_batch_lookup_handles_not_found() {
        let db = real_db();
        let codes = vec!["C1525".to_string(), "CNOTEXIST123".to_string()];
        let results = db.get_by_lcsc_batch(&codes).unwrap();
        assert!(results["C1525"].is_some());
        assert!(results["CNOTEXIST123"].is_none());
    }

    #[test]
    fn test_batch_lookup_dedupes_input() {
        let db = real_db();
        let codes = vec!["C1525".to_string(), "c1525".to_string(), "C1525".to_string()];
        let results = db.get_by_lcsc_batch(&codes).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results.contains_key("C1525"));
    }

    // --- TestMPNLookup ---
    #[test]
    fn test_exact_mpn_match() {
        let db = real_db();
        let results = db.get_by_mpn("AO3400A");
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r["model"] == "AO3400A"));
    }

    #[test]
    fn test_mpn_case_insensitive() {
        let db = real_db();
        let upper = db.get_by_mpn("AO3400A");
        let lower = db.get_by_mpn("ao3400a");
        assert_eq!(upper.len(), lower.len());
        if !upper.is_empty() {
            assert_eq!(upper[0]["lcsc"], lower[0]["lcsc"]);
        }
    }

    #[test]
    fn test_mpn_not_found() {
        let db = real_db();
        assert_eq!(db.get_by_mpn("TOTALLYFAKE12345XYZ"), Vec::<Value>::new());
    }

    #[test]
    fn test_mpn_empty_string() {
        let db = real_db();
        assert_eq!(db.get_by_mpn(""), Vec::<Value>::new());
        assert_eq!(db.get_by_mpn("   "), Vec::<Value>::new());
    }

    #[test]
    fn test_mpn_with_distributor_suffix() {
        let db = real_db();
        let base_results = db.get_by_mpn("AO3400A");
        if !base_results.is_empty() {
            // Non-crashing is the baseline, matching the Python test's own weak assertion —
            // AO3400A-TR may or may not be a separate MPN in the DB.
            let _tr_results = db.get_by_mpn("AO3400A-TR");
        }
    }

    #[test]
    fn test_mpn_returns_correct_fields() {
        let db = real_db();
        let results = db.get_by_mpn("AO3400A");
        if let Some(part) = results.first() {
            for field in ["lcsc", "model", "manufacturer", "package", "stock", "price", "library_type", "specs"] {
                assert!(part.get(field).is_some(), "missing field '{field}'");
            }
        }
    }

    // --- TestCategoriesClient ---
    #[test]
    fn get_categories_for_client_smoke_test() {
        let db = real_db();
        let categories = db.get_categories_for_client();
        assert!(!categories.is_empty(), "should list at least one category with parts");
        let first = &categories[0];
        assert!(first.get("id").is_some());
        assert!(first.get("name").is_some());
        assert!(first.get("subcategories").is_some());
    }

    // --- TestFindBySubcategory ---
    #[test]
    fn find_by_subcategory_smoke_test() {
        let db = real_db();
        let mosfet_id = db.resolve_subcategory_name("MOSFETs").unwrap();
        let results = db.find_by_subcategory(mosfet_id, None, None, 10, None, true, 5);
        assert!(!results.is_empty(), "should find MOSFETs in the subcategory");
        for part in &results {
            assert_eq!(part["subcategory_id"], mosfet_id);
        }
    }

    #[test]
    fn find_by_subcategory_primary_spec_numeric_filter() {
        let db = real_db();
        // Subcategory 2980 is Chip Resistor - Surface Mount in the fixture DBs used
        // elsewhere in this workspace (pcbparts-search's own tests); resolve by name
        // here instead of hardcoding an ID, since Phase 5 doesn't own that mapping.
        let resistor_id = db.resolve_subcategory_name("Chip Resistor").unwrap();
        let results = db.find_by_subcategory(resistor_id, Some("Resistance"), Some("10k"), 10, None, true, 10);
        // Non-crashing + correct subcategory is the baseline for this zero-Python-coverage path.
        for part in &results {
            assert_eq!(part["subcategory_id"], resistor_id);
        }
    }

    #[test]
    fn find_by_subcategory_categorical_spec_filter() {
        let db = real_db();
        // Use a categorical (non-numeric) spec to exercise the LIKE-pattern string-spec branch.
        // "Package" is a categorical spec that doesn't have a numeric parser.
        let resistor_id = db.resolve_subcategory_name("Chip Resistor").unwrap();
        let results = db.find_by_subcategory(resistor_id, Some("Package"), Some("0603"), 10, None, true, 10);
        // Non-crashing + correct subcategory is the baseline for this zero-Python-coverage path.
        // (The LIKE pattern currently doesn't match real data due to a pre-existing latent bug
        // in Python's own source, but the function should still execute without error.)
        for part in &results {
            assert_eq!(part["subcategory_id"], resistor_id);
        }
    }

    // --- TestListAttributes ---
    #[test]
    fn test_list_mosfet_attributes() {
        let db = real_db();
        let result = db.list_attributes(None, Some("MOSFETs"), 1000);
        assert!(result.get("error").is_none());
        assert_eq!(result["subcategory_name"], "MOSFETs");
        let attrs = result["attributes"].as_array().unwrap();
        assert!(attrs.len() > 5);

        let names: Vec<&str> = attrs.iter().map(|a| a["name"].as_str().unwrap()).collect();
        assert!(names.iter().any(|n| n.contains("Vgs") || n.contains("Gate")));
        assert!(names.iter().any(|n| n.contains("Type")));
    }

    #[test]
    fn test_list_attributes_includes_type_info() {
        let db = real_db();
        let result = db.list_attributes(None, Some("MOSFETs"), 1000);
        for attr in result["attributes"].as_array().unwrap() {
            let ty = attr["type"].as_str().unwrap();
            assert!(ty == "numeric" || ty == "string");
            assert!(attr.get("count").is_some());
        }
    }

    #[test]
    fn test_list_attributes_includes_aliases() {
        let db = real_db();
        let result = db.list_attributes(None, Some("MOSFETs"), 1000);
        let vgs_attr = result["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["name"].as_str().unwrap_or_default().contains("Gate Threshold"));
        if let Some(attr) = vgs_attr {
            assert_eq!(attr["alias"], "Vgs(th)");
        }
    }

    #[test]
    fn test_list_attributes_not_found() {
        let db = real_db();
        let result = db.list_attributes(None, Some("NonExistent12345"), 1000);
        assert!(result.get("error").is_some());
    }

    #[test]
    fn test_list_attributes_numeric_sort_for_height() {
        use pcbparts_parsers::parsers::parse_length_mm;
        let db = real_db();
        let result = db.list_attributes(Some(2965), None, 1000);
        let height = result["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["name"] == "Height - Seated (Max)")
            .expect("Height attribute should be present in subcat 2965");
        assert_eq!(height["type"], "numeric");

        let values = height["values"].as_array().unwrap();
        assert!(!values.is_empty(), "Height values should not be empty");
        let first = values[0].as_str().unwrap();
        let first_parsed = parse_length_mm(first).expect("first value should parse");
        assert!(first_parsed < 10.0, "first value should be under 10mm, got {first:?}");
        if let Some(dash_pos) = values.iter().position(|v| v == "-") {
            assert!(dash_pos > 0);
        }
    }

    #[test]
    fn test_list_attributes_numeric_min_max() {
        let db = real_db();
        let result = db.list_attributes(Some(2965), None, 1000);
        let height = result["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["name"] == "Height - Seated (Max)")
            .unwrap();
        let min = height["min"].as_f64().unwrap();
        let max = height["max"].as_f64().unwrap();
        assert!(min < max);
        assert!((2.0..=6.0).contains(&min));
        assert!((15.0..=40.0).contains(&max));
    }

    #[test]
    fn test_list_capacitor_attributes() {
        let db = real_db();
        let result = db.list_attributes(None, Some("MLCC"), 1000);
        assert!(result.get("error").is_none());
        let names: Vec<&str> = result["attributes"].as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap()).collect();
        assert!(names.iter().any(|n| n.contains("Capacitance")));
        assert!(names.iter().any(|n| n.contains("Voltage")));
    }
}
