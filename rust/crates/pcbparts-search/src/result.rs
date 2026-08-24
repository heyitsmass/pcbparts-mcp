use pcbparts_parsers::mounting::detect_mounting_type;
use rusqlite::Row;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct SubcategoryInfo {
    pub name: String,
    pub category_id: i64,
    pub category_name: Option<String>,
}

/// Convert a database row to a component dict, matching client.py's `_transform_part()` shape.
pub fn row_to_dict(row: &Row, subcategories: &BTreeMap<i64, SubcategoryInfo>) -> Value {
    let attributes: Option<String> = row.get("attributes").unwrap_or(None);
    let mut specs = serde_json::Map::new();
    if let Some(attrs_json) = attributes.filter(|s| !s.is_empty()) {
        if let Ok(pairs) = serde_json::from_str::<Vec<(String, String)>>(&attrs_json) {
            for (name, value) in pairs {
                specs.insert(name, Value::String(value));
            }
        }
        // Malformed JSON: continue with empty specs, matching Python's
        // `except (json.JSONDecodeError, TypeError)` fallback.
    }

    let library_type_code: Option<String> = row.get("library_type").unwrap_or(None);
    let library_type_value = match library_type_code.as_deref() {
        Some("b") => Value::String("basic".to_string()),
        Some("p") => Value::String("preferred".to_string()),
        Some("e") => Value::String("extended".to_string()),
        Some(other) => Value::String(other.to_string()),
        None => Value::Null,
    };
    let preferred = library_type_code.as_deref().map(|code| code == "b" || code == "p").unwrap_or(false);

    let stock: Option<i64> = row.get("stock").unwrap_or(None);
    let subcategory_id: Option<i64> = row.get("subcategory_id").unwrap_or(None);
    let subcat_info = subcategory_id.and_then(|id| subcategories.get(&id));
    let package: Option<String> = row.get("package").unwrap_or(None);
    let category = subcat_info.and_then(|i| i.category_name.clone());
    let subcategory = subcat_info.map(|i| i.name.clone());

    json!({
        "lcsc": row.get::<_, String>("lcsc").unwrap_or_default(),
        "model": row.get::<_, Option<String>>("mpn").unwrap_or(None),
        "manufacturer": row.get::<_, Option<String>>("manufacturer").unwrap_or(None),
        "package": package,
        "stock": stock,
        "price": row.get::<_, Option<f64>>("price").unwrap_or(None),
        "price_10": Value::Null,
        "library_type": library_type_value,
        "preferred": preferred,
        "category": category,
        "subcategory": subcategory,
        "subcategory_id": subcategory_id,
        "mounting_type": detect_mounting_type(package.as_deref(), category.as_deref(), subcategory.as_deref()),
        "description": row.get::<_, Option<String>>("description").unwrap_or(None),
        "specs": Value::Object(specs),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE components (
                lcsc TEXT, mpn TEXT, manufacturer TEXT, package TEXT, stock INTEGER,
                library_type TEXT, subcategory_id INTEGER, price REAL, description TEXT, attributes TEXT
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO components (lcsc, mpn, manufacturer, package, stock, library_type, subcategory_id, price, description, attributes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                "C25804", "0603WAF1002T5E", "UNI-ROYAL(Uniroyal Elec)", "0603", 15959990i64,
                "b", 2980i64, 0.0067, "-55℃~+155℃ 100mW 10kΩ 75V Thick Film Resistor ±1% ±100ppm/℃ 0603 Chip Resistor - Surface Mount ROHS",
                r#"[["Resistance", "10kΩ"], ["Operating Temperature", "-55℃~+155℃"], ["Power(Watts)", "100mW"], ["Type", "Thick Film Resistor"], ["Voltage-Supply(Max)", "75V"], ["Tolerance", "±1%"], ["Temperature Coefficient", "±100ppm/℃"]]"#,
            ],
        )
        .unwrap();
        conn
    }

    fn test_subcategories() -> BTreeMap<i64, SubcategoryInfo> {
        BTreeMap::from([(2980, SubcategoryInfo {
            name: "Chip Resistor - Surface Mount".to_string(),
            category_id: 10,
            category_name: Some("Resistors".to_string()),
        })])
    }

    #[test]
    fn test_row_to_dict_full_shape() {
        let conn = test_conn();
        let subcategories = test_subcategories();
        let mut stmt = conn.prepare("SELECT * FROM components WHERE lcsc = 'C25804'").unwrap();
        let mut rows = stmt.query([]).unwrap();
        let row = rows.next().unwrap().unwrap();
        let dict = row_to_dict(row, &subcategories);

        assert_eq!(dict["lcsc"], "C25804");
        assert_eq!(dict["model"], "0603WAF1002T5E");
        assert_eq!(dict["manufacturer"], "UNI-ROYAL(Uniroyal Elec)");
        assert_eq!(dict["package"], "0603");
        assert_eq!(dict["stock"], 15959990);
        assert_eq!(dict["price"], 0.0067);
        assert_eq!(dict["price_10"], serde_json::Value::Null);
        assert_eq!(dict["library_type"], "basic");
        assert_eq!(dict["preferred"], true);
        assert_eq!(dict["category"], "Resistors");
        assert_eq!(dict["subcategory"], "Chip Resistor - Surface Mount");
        assert_eq!(dict["subcategory_id"], 2980);
        assert_eq!(dict["mounting_type"], "smd");
        assert_eq!(dict["specs"]["Resistance"], "10kΩ");
        assert_eq!(dict["specs"]["Tolerance"], "±1%");

        // Key order must be preserved (JSON array order), not alphabetized —
        // requires the `preserve_order` serde_json feature (Global Constraints).
        let keys: Vec<&String> = dict["specs"].as_object().unwrap().keys().collect();
        assert_eq!(
            keys,
            vec!["Resistance", "Operating Temperature", "Power(Watts)", "Type", "Voltage-Supply(Max)", "Tolerance", "Temperature Coefficient"]
        );
    }

    #[test]
    fn test_row_to_dict_library_type_mapping() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE components (
                lcsc TEXT, mpn TEXT, manufacturer TEXT, package TEXT, stock INTEGER,
                library_type TEXT, subcategory_id INTEGER, price REAL, description TEXT, attributes TEXT
            );
            INSERT INTO components VALUES ('C1', 'M1', 'Mfr', 'SOT-23', 100, 'e', 999, 0.1, 'desc', NULL);",
        )
        .unwrap();
        let subcategories: BTreeMap<i64, SubcategoryInfo> = BTreeMap::new();
        let mut stmt = conn.prepare("SELECT * FROM components").unwrap();
        let mut rows = stmt.query([]).unwrap();
        let row = rows.next().unwrap().unwrap();
        let dict = row_to_dict(row, &subcategories);

        assert_eq!(dict["library_type"], "extended");
        assert_eq!(dict["preferred"], false);
        assert_eq!(dict["category"], serde_json::Value::Null);
        assert_eq!(dict["subcategory"], serde_json::Value::Null);
        assert_eq!(dict["specs"], serde_json::json!({}));
    }

    #[test]
    fn test_row_to_dict_null_columns() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE components (
                lcsc TEXT, mpn TEXT, manufacturer TEXT, package TEXT, stock INTEGER,
                library_type TEXT, subcategory_id INTEGER, price REAL, description TEXT, attributes TEXT
            );
            INSERT INTO components VALUES ('C2', 'M2', 'Mfr2', 'PKG', NULL, NULL, NULL, 0.05, 'desc2', NULL);",
        )
        .unwrap();
        let subcategories: BTreeMap<i64, SubcategoryInfo> = BTreeMap::new();
        let mut stmt = conn.prepare("SELECT * FROM components").unwrap();
        let mut rows = stmt.query([]).unwrap();
        let row = rows.next().unwrap().unwrap();
        let dict = row_to_dict(row, &subcategories);

        // When columns are NULL, corresponding JSON fields should be null, not default values
        assert_eq!(dict["stock"], serde_json::Value::Null);
        assert_eq!(dict["library_type"], serde_json::Value::Null);
        assert_eq!(dict["preferred"], false);
        assert_eq!(dict["subcategory_id"], serde_json::Value::Null);
        assert_eq!(dict["category"], serde_json::Value::Null);
        assert_eq!(dict["subcategory"], serde_json::Value::Null);
    }
}
