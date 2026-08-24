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
use pcbparts_search::spec_filter::SpecFilter;
use rusqlite::Connection;
use serde_json::{json, Value};
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

/// Mirrors Python's `ComponentDatabase.search()` defaults exactly — the real
/// two-layer default architecture this crate's `SearchArgs` completes.
/// `pcbparts_search::engine::SearchParams::default()` uses Rust-idiomatic
/// `0`/`"relevance"` for direct `SearchEngine` callers; this struct is the layer
/// that actually applies Python's `min_stock=10`/`sort_by="stock"` defaults.
pub struct SearchArgs {
    pub query: Option<String>,
    pub subcategory_id: Option<i64>,
    pub subcategory_name: Option<String>,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub spec_filters: Vec<SpecFilter>,
    pub library_type: Option<String>,
    pub prefer_no_fee: bool,
    pub min_stock: i64,
    pub package: Option<String>,
    pub packages: Option<Vec<String>>,
    pub manufacturer: Option<String>,
    pub mounting_type: Option<String>,
    pub match_all_terms: bool,
    pub sort_by: String,
    pub limit: i64,
    pub offset: i64,
}

impl Default for SearchArgs {
    fn default() -> Self {
        Self {
            query: None,
            subcategory_id: None,
            subcategory_name: None,
            category_id: None,
            category_name: None,
            spec_filters: Vec::new(),
            library_type: None,
            prefer_no_fee: true,
            min_stock: DEFAULT_MIN_STOCK,
            package: None,
            packages: None,
            manufacturer: None,
            mounting_type: None,
            match_all_terms: true,
            sort_by: "stock".to_string(),
            limit: 50,
            offset: 0,
        }
    }
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

    pub fn get_stats(&self) -> Value {
        let conn = self.conn.lock().unwrap();
        stats::get_stats(&conn, &self.categories, &self.subcategories)
    }

    pub fn search(&self, args: SearchArgs) -> Value {
        let conn = self.conn.lock().unwrap();
        // Clamp min_stock to match database reality (database only has parts with
        // stock >= DEFAULT_MIN_STOCK) — prevents misleading searches where users think
        // they can find 0-stock parts.
        let original_min_stock = args.min_stock;
        let clamped_min_stock = args.min_stock.max(DEFAULT_MIN_STOCK);

        let mut result = self.search_engine.search(
            &conn,
            SearchParams {
                query: args.query,
                subcategory_id: args.subcategory_id,
                subcategory_name: args.subcategory_name,
                category_id: args.category_id,
                category_name: args.category_name,
                spec_filters: args.spec_filters,
                library_type: args.library_type,
                prefer_no_fee: args.prefer_no_fee,
                min_stock: clamped_min_stock,
                package: args.package,
                packages: args.packages,
                manufacturer: args.manufacturer,
                mounting_type: args.mounting_type,
                match_all_terms: args.match_all_terms,
                sort_by: args.sort_by,
                limit: args.limit,
                offset: args.offset,
            },
        );

        if original_min_stock < DEFAULT_MIN_STOCK {
            result["warning"] = json!(format!(
                "Database only contains parts with stock >= {DEFAULT_MIN_STOCK}. \
                 Requested min_stock={original_min_stock} was increased to {DEFAULT_MIN_STOCK}. \
                 Use jlc_stock_check tool for low-stock or out-of-stock parts."
            ));
        }

        result
    }

    pub fn expand_package(&self, package: &str) -> Vec<String> {
        pcbparts_search::resolvers::expand_package(package)
    }

    pub fn resolve_manufacturer(&self, name: &str) -> String {
        pcbparts_search::resolvers::resolve_manufacturer(name)
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

    // --- TestDatabaseStats ---
    #[test]
    fn test_get_stats() {
        let db = real_db();
        let stats = db.get_stats();

        assert!(stats["total_parts"].as_i64().unwrap() > 0);

        let by_lib = &stats["by_library_type"];
        assert!(by_lib.get("basic").is_some());
        assert!(by_lib.get("preferred").is_some());
        assert!(by_lib.get("extended").is_some());

        assert!(stats["subcategories"].as_i64().unwrap() > 0);
    }

    fn search_args() -> SearchArgs {
        SearchArgs::default()
    }

    // --- TestShortestMatchPriority (remainder) ---
    #[test]
    fn test_search_shows_resolved_name() {
        let db = real_db();
        let result = db.search(SearchArgs { subcategory_name: Some("crystal".to_string()), limit: 1, ..search_args() });
        assert!(result.get("error").is_none());
        assert_eq!(result["filters_applied"]["subcategory_resolved"], "Crystals");
    }

    // --- TestSearchWithNames ---
    #[test]
    fn test_search_by_subcategory_name() {
        let db = real_db();
        let result = db.search(SearchArgs { subcategory_name: Some("MOSFETs".to_string()), limit: 5, ..search_args() });
        assert!(result.get("error").is_none());
        assert!(result["total"].as_i64().unwrap() > 0);
        assert!(result["results"].as_array().unwrap().len() <= 5);
        assert_eq!(result["filters_applied"]["subcategory_name"], "MOSFETs");
        assert!(result["filters_applied"]["subcategory_id"].is_i64());
    }

    #[test]
    fn test_search_by_subcategory_name_not_found() {
        let db = real_db();
        let result = db.search(SearchArgs { subcategory_name: Some("NonExistent12345".to_string()), limit: 5, ..search_args() });
        assert!(result.get("error").is_some());
        assert!(result["error"].as_str().unwrap().to_lowercase().contains("not found"));
        assert_eq!(result["total"], 0);
    }

    #[test]
    fn test_search_subcategory_id_takes_precedence() {
        let db = real_db();
        let mosfet_id = db.resolve_subcategory_name("MOSFETs").unwrap();
        let result = db.search(SearchArgs {
            subcategory_id: Some(mosfet_id),
            subcategory_name: Some("Resistors".to_string()),
            limit: 5,
            ..search_args()
        });
        assert!(result.get("error").is_none());
        assert_eq!(result["filters_applied"]["subcategory_id"], mosfet_id);
    }

    // --- TestLibraryTypeAndPreference ---
    #[test]
    fn test_prefer_no_fee_sorts_basic_first() {
        let db = real_db();
        let result = db.search(SearchArgs { prefer_no_fee: true, limit: 50, ..search_args() });
        assert!(result.get("error").is_none());
        assert!(result["total"].as_i64().unwrap() > 0);

        let types_seen: Vec<String> = result["results"].as_array().unwrap().iter().map(|p| p["library_type"].as_str().unwrap().to_string()).collect();
        if types_seen.contains(&"basic".to_string()) && types_seen.contains(&"extended".to_string()) {
            let first_basic = types_seen.iter().position(|t| t == "basic").unwrap();
            let first_extended = types_seen.iter().position(|t| t == "extended").unwrap();
            assert!(first_basic < first_extended);
        }
        if types_seen.contains(&"preferred".to_string()) && types_seen.contains(&"extended".to_string()) {
            let first_preferred = types_seen.iter().position(|t| t == "preferred").unwrap();
            let first_extended = types_seen.iter().position(|t| t == "extended").unwrap();
            assert!(first_preferred < first_extended);
        }
    }

    #[test]
    fn test_prefer_no_fee_includes_all_types() {
        let db = real_db();
        let result = db.search(SearchArgs { prefer_no_fee: true, limit: 500, ..search_args() });
        assert!(result.get("error").is_none());
        let basic = result["library_type_counts"]["basic"].as_i64().unwrap_or(0);
        let preferred = result["library_type_counts"]["preferred"].as_i64().unwrap_or(0);
        let total = result["total"].as_i64().unwrap();
        assert!(total > basic + preferred || total > 100);
    }

    #[test]
    fn test_prefer_no_fee_false_no_sorting_preference() {
        let db = real_db();
        let result = db.search(SearchArgs { prefer_no_fee: false, limit: 50, ..search_args() });
        assert!(result.get("error").is_none());
        assert!(result["total"].as_i64().unwrap() > 0);
        assert_eq!(result["filters_applied"]["prefer_no_fee"], false);
    }

    #[test]
    fn test_prefer_no_fee_default_is_true() {
        let db = real_db();
        let result = db.search(SearchArgs { limit: 10, ..search_args() });
        assert!(result.get("error").is_none());
        assert_eq!(result["filters_applied"]["prefer_no_fee"], true);
    }

    #[test]
    fn test_basic_filter_excludes_others() {
        let db = real_db();
        let result = db.search(SearchArgs { library_type: Some("basic".to_string()), limit: 50, ..search_args() });
        assert!(result.get("error").is_none());
        for part in result["results"].as_array().unwrap() {
            assert_eq!(part["library_type"], "basic");
        }
    }

    #[test]
    fn test_extended_filter_excludes_others() {
        let db = real_db();
        let result = db.search(SearchArgs { library_type: Some("extended".to_string()), limit: 10, ..search_args() });
        assert!(result.get("error").is_none());
        for part in result["results"].as_array().unwrap() {
            assert_eq!(part["library_type"], "extended");
        }
    }

    #[test]
    fn test_library_type_filter_with_prefer_no_fee() {
        let db = real_db();
        let result = db.search(SearchArgs { library_type: Some("basic".to_string()), prefer_no_fee: true, limit: 10, ..search_args() });
        assert!(result.get("error").is_none());
        for part in result["results"].as_array().unwrap() {
            assert_eq!(part["library_type"], "basic");
        }
    }

    #[test]
    fn test_no_fee_filter_excludes_extended() {
        let db = real_db();
        let result = db.search(SearchArgs { library_type: Some("no_fee".to_string()), limit: 50, ..search_args() });
        assert!(result.get("error").is_none());
        assert!(result["total"].as_i64().unwrap() > 0);
        for part in result["results"].as_array().unwrap() {
            let lt = part["library_type"].as_str().unwrap();
            assert!(lt == "basic" || lt == "preferred", "no_fee returned {lt} part");
        }
    }

    // --- TestPackageFilters ---
    #[test]
    fn test_single_package_filter() {
        let db = real_db();
        let result = db.search(SearchArgs { subcategory_name: Some("Chip Resistor".to_string()), package: Some("0603".to_string()), limit: 10, ..search_args() });
        assert!(result.get("error").is_none());
        for part in result["results"].as_array().unwrap() {
            assert_eq!(part["package"], "0603");
        }
    }

    #[test]
    fn test_multiple_packages_or_logic() {
        let db = real_db();
        let result = db.search(SearchArgs {
            subcategory_name: Some("Chip Resistor".to_string()),
            packages: Some(vec!["0402".to_string(), "0603".to_string(), "0805".to_string()]),
            limit: 20,
            ..search_args()
        });
        assert!(result.get("error").is_none());
        let mut found = std::collections::HashSet::new();
        for part in result["results"].as_array().unwrap() {
            let pkg = part["package"].as_str().unwrap();
            assert!(["0402", "0603", "0805"].contains(&pkg));
            found.insert(pkg.to_string());
        }
        assert!(found.len() >= 2);
    }

    // --- TestFTSSearch ---
    #[test]
    fn test_single_word_query() {
        let db = real_db();
        let result = db.search(SearchArgs { query: Some("ESP32".to_string()), limit: 10, ..search_args() });
        assert!(result.get("error").is_none());
        assert!(result["total"].as_i64().unwrap() > 0);
    }

    #[test]
    fn test_multi_word_query() {
        let db = real_db();
        let result = db.search(SearchArgs { subcategory_id: Some(2929), query: Some("10uF 25V".to_string()), limit: 10, ..search_args() });
        assert!(result.get("error").is_none());
        assert!(result["total"].as_i64().unwrap() > 0);
    }

    #[test]
    fn test_fts_with_spec_filter() {
        let db = real_db();
        let result = db.search(SearchArgs {
            subcategory_name: Some("MOSFETs".to_string()),
            query: Some("AO3400".to_string()),
            spec_filters: vec![SpecFilter::new("Vgs(th)", "<", "2V").unwrap()],
            limit: 10,
            ..search_args()
        });
        assert!(result.get("error").is_none());
    }

    // --- TestFTSOrMode ---
    #[test]
    fn test_match_all_terms_default_is_true() {
        let db = real_db();
        let result = db.search(SearchArgs { query: Some("test".to_string()), limit: 1, ..search_args() });
        assert_eq!(result["filters_applied"]["match_all_terms"], true);
    }

    #[test]
    fn test_or_mode_returns_more_results() {
        let db = real_db();
        let result_and = db.search(SearchArgs { query: Some("hall effect".to_string()), match_all_terms: true, limit: 1, ..search_args() });
        let result_or = db.search(SearchArgs { query: Some("hall effect".to_string()), match_all_terms: false, limit: 1, ..search_args() });
        assert!(result_or["total"].as_i64().unwrap() >= result_and["total"].as_i64().unwrap());
    }

    // --- TestLibraryTypeCounts ---
    #[test]
    fn test_response_includes_library_type_counts() {
        let db = real_db();
        let result = db.search(SearchArgs { subcategory_name: Some("MOSFETs".to_string()), limit: 1, ..search_args() });
        let counts = &result["library_type_counts"];
        assert!(counts.get("basic").is_some());
        assert!(counts.get("preferred").is_some());
        assert!(counts.get("extended").is_some());
    }

    #[test]
    fn test_response_includes_no_fee_available() {
        let db = real_db();
        let result = db.search(SearchArgs { subcategory_name: Some("MOSFETs".to_string()), limit: 1, ..search_args() });
        assert!(result.get("no_fee_available").is_some());
        assert_eq!(result["no_fee_available"], true);
    }

    #[test]
    fn test_usb_c_has_no_basic_parts() {
        let db = real_db();
        let result = db.search(SearchArgs { subcategory_name: Some("USB Connectors".to_string()), query: Some("TYPE-C".to_string()), limit: 1, ..search_args() });
        assert_eq!(result["library_type_counts"]["basic"], 0);
        assert_eq!(result["library_type_counts"]["preferred"], 0);
        assert_eq!(result["no_fee_available"], false);
    }

    // --- TestErrorMessagesWithSuggestions ---
    #[test]
    fn test_not_found_includes_similar_subcategories() {
        let db = real_db();
        let result = db.search(SearchArgs { subcategory_name: Some("usb type c connector xyz".to_string()), limit: 1, ..search_args() });
        assert!(result.get("error").is_some());
        assert!(result.get("similar_subcategories").is_some());
    }

    #[test]
    fn test_error_response_has_consistent_structure() {
        let db = real_db();
        let result = db.search(SearchArgs { subcategory_name: Some("nonexistent12345".to_string()), limit: 1, ..search_args() });
        assert!(result.get("error").is_some());
        assert_eq!(result["total"], 0);
        assert!(result.get("library_type_counts").is_some());
        assert!(result.get("no_fee_available").is_some());
    }

    // --- TestSpecFilters ---
    #[test]
    fn test_vgs_threshold_filter() {
        use pcbparts_parsers::parsers::parse_voltage;
        let db = real_db();
        let result = db.search(SearchArgs {
            subcategory_name: Some("MOSFETs".to_string()),
            spec_filters: vec![SpecFilter::new("Vgs(th)", "<", "2V").unwrap()],
            limit: 10,
            ..search_args()
        });
        assert!(result.get("error").is_none());
        assert!(result["total"].as_i64().unwrap() > 0);
        for part in result["results"].as_array().unwrap() {
            if let Some(vgs) = part["specs"].get("Gate Threshold Voltage (Vgs(th))").and_then(|v| v.as_str()) {
                let parsed = parse_voltage(vgs);
                assert!(parsed.is_some() && parsed.unwrap() < 2.0, "Vgs(th)={vgs} should be < 2V");
            }
        }
    }

    #[test]
    fn test_capacitor_voltage_filter() {
        use pcbparts_parsers::parsers::parse_voltage;
        let db = real_db();
        let result = db.search(SearchArgs {
            subcategory_id: Some(2929),
            query: Some("10uF".to_string()),
            spec_filters: vec![SpecFilter::new("Voltage", ">=", "25V").unwrap()],
            limit: 10,
            ..search_args()
        });
        assert!(result.get("error").is_none());
        assert!(result["total"].as_i64().unwrap() > 0);
        for part in result["results"].as_array().unwrap() {
            if let Some(v) = part["specs"].get("Voltage Rating").and_then(|v| v.as_str()) {
                let parsed = parse_voltage(v);
                assert!(parsed.is_some() && parsed.unwrap() >= 25.0, "Voltage={v} should be >= 25V");
            }
        }
    }

    #[test]
    fn test_multiple_spec_filters() {
        let db = real_db();
        let result = db.search(SearchArgs {
            subcategory_name: Some("MOSFETs".to_string()),
            spec_filters: vec![SpecFilter::new("Vgs(th)", "<", "2V").unwrap(), SpecFilter::new("Id", ">=", "3A").unwrap()],
            limit: 10,
            ..search_args()
        });
        assert!(result.get("error").is_none());
        assert!(result["total"].as_i64().unwrap() >= 0);
    }

    #[test]
    fn test_height_le_filter_excludes_taller() {
        use pcbparts_parsers::parsers::{parse_dimensions_from_package, parse_length_mm};
        let db = real_db();
        let result = db.search(SearchArgs {
            subcategory_id: Some(2965),
            spec_filters: vec![SpecFilter::new("Height - Seated (Max)", "<=", "5.4mm").unwrap()],
            limit: 25,
            ..search_args()
        });
        assert!(result.get("error").is_none());
        let results = result["results"].as_array().unwrap();
        assert!(!results.is_empty(), "should find at least one <=5.4mm part");
        for part in results {
            let height = part["specs"].get("Height - Seated (Max)").and_then(|v| v.as_str());
            let mut parsed = height.and_then(parse_length_mm);
            if parsed.is_none() {
                let (_, h) = parse_dimensions_from_package(part["package"].as_str().unwrap_or(""));
                parsed = h;
            }
            assert!(parsed.is_some() && parsed.unwrap() <= 5.4 + 1e-6, "Part {} leaked with Height={height:?}", part["lcsc"]);
        }
    }

    #[test]
    fn test_diameter_ge_filter_excludes_smaller() {
        use pcbparts_parsers::parsers::{parse_dimensions_from_package, parse_length_mm};
        let db = real_db();
        let result = db.search(SearchArgs {
            subcategory_id: Some(2965),
            spec_filters: vec![SpecFilter::new("Diameter", ">=", "10mm").unwrap()],
            limit: 25,
            ..search_args()
        });
        assert!(result.get("error").is_none());
        let results = result["results"].as_array().unwrap();
        assert!(!results.is_empty());
        for part in results {
            let diameter = part["specs"].get("Diameter").and_then(|v| v.as_str());
            let mut parsed = diameter.and_then(parse_length_mm);
            if parsed.is_none() {
                let (d, _) = parse_dimensions_from_package(part["package"].as_str().unwrap_or(""));
                parsed = d;
            }
            assert!(parsed.is_some() && parsed.unwrap() >= 10.0 - 1e-6, "Part {} leaked with Diameter={diameter:?}", part["lcsc"]);
        }
    }

    #[test]
    fn test_height_and_diameter_combined() {
        use pcbparts_parsers::parsers::{parse_dimensions_from_package, parse_length_mm, parse_capacitance};
        let db = real_db();
        let result = db.search(SearchArgs {
            subcategory_id: Some(2965),
            spec_filters: vec![
                SpecFilter::new("Height - Seated (Max)", "<=", "5.4mm").unwrap(),
                SpecFilter::new("Diameter", "<=", "6.3mm").unwrap(),
                SpecFilter::new("Capacitance", ">=", "220uF").unwrap(),
                SpecFilter::new("Voltage", ">=", "16V").unwrap(),
            ],
            limit: 25,
            ..search_args()
        });
        assert!(result.get("error").is_none());
        for part in result["results"].as_array().unwrap() {
            let mut h = part["specs"].get("Height - Seated (Max)").and_then(|v| v.as_str()).and_then(parse_length_mm);
            let mut d = part["specs"].get("Diameter").and_then(|v| v.as_str()).and_then(parse_length_mm);
            if h.is_none() || d.is_none() {
                let (pkg_d, pkg_h) = parse_dimensions_from_package(part["package"].as_str().unwrap_or(""));
                h = h.or(pkg_h);
                d = d.or(pkg_d);
            }
            let c = part["specs"].get("Capacitance").and_then(|v| v.as_str()).and_then(parse_capacitance);
            assert!(h.is_some() && h.unwrap() <= 5.4 + 1e-6, "{}: height {h:?}", part["lcsc"]);
            assert!(d.is_some() && d.unwrap() <= 6.3 + 1e-6, "{}: diameter {d:?}", part["lcsc"]);
            assert!(c.is_some() && c.unwrap() >= 220e-6 * (1.0 - 1e-6), "{}: capacitance {c:?}", part["lcsc"]);
        }
    }

    #[test]
    fn test_height_rescued_from_package_string() {
        let db = real_db();
        let result = db.search(SearchArgs {
            subcategory_id: Some(2965),
            package: Some("SMD,D6.3xL5.8mm".to_string()),
            spec_filters: vec![SpecFilter::new("Height - Seated (Max)", "<=", "6mm").unwrap()],
            limit: 50,
            min_stock: 0,
            ..search_args()
        });
        assert!(result.get("error").is_none());
        let results = result["results"].as_array().unwrap();
        let lcscs: Vec<&str> = results.iter().map(|p| p["lcsc"].as_str().unwrap()).collect();
        assert!(lcscs.contains(&"C729678"), "C729678 should be rescued by the package-string fallback");
        assert!(results.iter().any(|p| p["specs"].get("Height - Seated (Max)").map(|v| v == "-").unwrap_or(false)));
    }

    #[test]
    fn test_null_height_not_rescued_when_package_lacks_dims() {
        let db = real_db();
        let result = db.search(SearchArgs {
            subcategory_id: Some(2965),
            package: Some("SMD".to_string()),
            spec_filters: vec![SpecFilter::new("Height - Seated (Max)", "<=", "100mm").unwrap()],
            limit: 100,
            min_stock: 0,
            ..search_args()
        });
        assert!(result.get("error").is_none());
        let lcscs: Vec<&str> = result["results"].as_array().unwrap().iter().map(|p| p["lcsc"].as_str().unwrap()).collect();
        assert!(!lcscs.contains(&"C18214363"), "C18214363 has no recoverable dims and must be dropped");
    }

    #[test]
    fn test_multiple_interface_values_use_or_logic() {
        use pcbparts_smart_parser::parse_smart_query;
        let parsed = parse_smart_query("sensor I2C SPI");
        let interface_filters: Vec<_> = parsed.spec_filters.iter().filter(|f| f.name == "Interface").collect();
        assert_eq!(interface_filters.len(), 2);
        let values: std::collections::HashSet<&str> = interface_filters.iter().map(|f| f.value.as_str()).collect();
        assert_eq!(values, std::collections::HashSet::from(["I2C", "SPI"]));
    }

    // --- TestSpecFilterValidation ---
    #[test]
    fn test_valid_operators_accepted() {
        for op in ["=", ">=", "<=", ">", "<"] {
            let sf = SpecFilter::new("Resistance", op, "10k").unwrap();
            assert_eq!(sf.operator.as_str(), op);
        }
    }

    #[test]
    fn test_not_equal_operator_rejected() {
        assert!(SpecFilter::new("Resistance", "!=", "10k").is_err());
    }

    #[test]
    fn test_invalid_operator_rejected() {
        assert!(SpecFilter::new("Resistance", "~=", "10k").is_err());
    }

    // --- TestPackageFamilyExpansion ---
    #[test]
    fn test_sot23_expands_to_variants() {
        let db = real_db();
        let expanded = db.expand_package("SOT-23");
        assert!(expanded.contains(&"SOT-23".to_string()));
        assert!(expanded.contains(&"SOT-23-3".to_string()));
        assert!(expanded.len() > 1);
    }

    #[test]
    fn test_specific_package_no_expansion() {
        let db = real_db();
        assert_eq!(db.expand_package("QFN-24-EP(4x4)"), vec!["QFN-24-EP(4x4)".to_string()]);
    }

    #[test]
    fn test_package_filter_uses_expansion() {
        let db = real_db();
        let result = db.search(SearchArgs { subcategory_name: Some("MOSFETs".to_string()), package: Some("SOT-23".to_string()), limit: 1, ..search_args() });
        assert!(result["total"].as_i64().unwrap() > 0);
    }

    #[test]
    fn test_so8_expands_to_all_variants() {
        let db = real_db();
        for pkg in ["SO-8", "SOP-8", "SOIC-8", "so8", "sop8", "soic8"] {
            let expanded = db.expand_package(pkg);
            assert!(expanded.contains(&"SO-8".to_string()), "{pkg} should expand to include SO-8");
            assert!(expanded.contains(&"SOP-8".to_string()), "{pkg} should expand to include SOP-8");
            assert!(expanded.contains(&"SOIC-8".to_string()), "{pkg} should expand to include SOIC-8");
        }
    }

    #[test]
    fn test_so8_search_finds_all_variants() {
        let db = real_db();
        let result = db.search(SearchArgs { subcategory_name: Some("MOSFETs".to_string()), package: Some("SO-8".to_string()), limit: 100, ..search_args() });
        assert!(result["total"].as_i64().unwrap() > 0);
        let found: std::collections::HashSet<&str> = result["results"].as_array().unwrap().iter().map(|p| p["package"].as_str().unwrap()).collect();
        let so_variants: std::collections::HashSet<&str> = found.intersection(&std::collections::HashSet::from(["SO-8", "SOP-8", "SOIC-8"])).cloned().collect();
        assert!(!so_variants.is_empty(), "Expected SO/SOP/SOIC-8, found {found:?}");
    }

    // --- TestStringSpecFilter ---
    #[test]
    fn test_type_filter_n_channel() {
        let db = real_db();
        let result = db.search(SearchArgs {
            subcategory_name: Some("MOSFETs".to_string()),
            spec_filters: vec![SpecFilter::new("Type", "=", "N-Channel").unwrap()],
            limit: 5,
            ..search_args()
        });
        assert!(result["total"].as_i64().unwrap() > 0);
        for part in result["results"].as_array().unwrap() {
            assert_eq!(part["specs"]["Type"], "N-Channel");
        }
    }

    #[test]
    fn test_type_filter_p_channel() {
        let db = real_db();
        let result = db.search(SearchArgs {
            subcategory_name: Some("MOSFETs".to_string()),
            spec_filters: vec![SpecFilter::new("Type", "=", "P-Channel").unwrap()],
            limit: 5,
            ..search_args()
        });
        assert!(result["total"].as_i64().unwrap() > 0);
        for part in result["results"].as_array().unwrap() {
            assert_eq!(part["specs"]["Type"], "P-Channel");
        }
    }

    // --- TestSearchSmartParsing ---
    #[test]
    fn test_search_resistor() {
        use pcbparts_smart_parser::parse_smart_query;
        let db = real_db();
        let parsed = parse_smart_query("10k resistor 0603 1%");
        let result = db.search(SearchArgs {
            subcategory_name: parsed.subcategory.clone(),
            package: parsed.package.clone(),
            spec_filters: parsed.spec_filters.clone(),
            limit: 5,
            ..search_args()
        });
        assert!(result["total"].as_i64().unwrap() > 0);
        for part in result["results"].as_array().unwrap() {
            assert!(part["subcategory"].as_str().unwrap().to_lowercase().contains("resistor"));
        }
    }

    #[test]
    fn test_search_mosfet() {
        use pcbparts_smart_parser::parse_smart_query;
        let db = real_db();
        let parsed = parse_smart_query("mosfet SOT-23");
        let result = db.search(SearchArgs { subcategory_name: parsed.subcategory.clone(), package: parsed.package.clone(), limit: 5, ..search_args() });
        assert!(result["total"].as_i64().unwrap() > 0);
        for part in result["results"].as_array().unwrap() {
            let pkg = part["package"].as_str().unwrap().to_lowercase();
            assert!(pkg.contains("sot-23") || pkg.contains("sot23"));
        }
    }
}
