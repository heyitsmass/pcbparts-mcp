//! Database statistics — ported 1:1 from `db/stats.py`.
use pcbparts_search::engine::CategoryInfo;
use pcbparts_search::result::SubcategoryInfo;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn get_stats(conn: &Connection, categories: &BTreeMap<i64, CategoryInfo>, subcategories: &BTreeMap<i64, SubcategoryInfo>) -> Value {
    let total_parts: i64 = conn.query_row("SELECT COUNT(*) FROM components", [], |r| r.get(0)).unwrap();

    let mut lib_counts = serde_json::Map::new();
    {
        let mut stmt = conn.prepare("SELECT library_type, COUNT(*) as cnt FROM components GROUP BY library_type").unwrap();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let code: Option<String> = row.get(0).unwrap();
            let count: i64 = row.get(1).unwrap();
            let name = match code.as_deref() {
                Some("b") => "basic",
                Some("p") => "preferred",
                Some("e") => "extended",
                Some(other) => other,
                None => "unknown",
            };
            lib_counts.insert(name.to_string(), json!(count));
        }
    }

    json!({
        "total_parts": total_parts,
        "by_library_type": lib_counts,
        "categories": categories.len(),
        "subcategories": subcategories.len(),
    })
}
