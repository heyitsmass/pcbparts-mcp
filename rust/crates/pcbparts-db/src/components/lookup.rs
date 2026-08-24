//! Component lookup functions — ported 1:1 from `db/lookup.py`.
use pcbparts_search::mpn::normalize_mpn;
use pcbparts_search::result::{row_to_dict, SubcategoryInfo};
use rusqlite::Connection;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

pub fn get_by_mpn(conn: &Connection, mpn: &str, subcategories: &BTreeMap<i64, SubcategoryInfo>) -> Vec<Value> {
    let mpn = mpn.trim();
    if mpn.is_empty() {
        return Vec::new();
    }

    let mut results: Vec<Value> = Vec::new();
    let mut seen_lcsc: HashSet<String> = HashSet::new();

    // 1. Exact match on mpn column (case-insensitive)
    {
        let mut stmt = conn
            .prepare("SELECT * FROM components WHERE LOWER(mpn) = LOWER(?1) ORDER BY stock DESC")
            .unwrap();
        let rows = stmt
            .query_map([mpn], |row| Ok(row_to_dict(row, subcategories)))
            .unwrap();
        for part in rows.filter_map(|r| r.ok()) {
            let lcsc = part["lcsc"].as_str().unwrap_or_default().to_string();
            seen_lcsc.insert(lcsc);
            results.push(part);
        }
    }

    // 2. Normalized MPN variants (strip -TR, insert T, etc.)
    if results.is_empty() {
        for variant in normalize_mpn(mpn) {
            let mut stmt = conn
                .prepare("SELECT * FROM components WHERE LOWER(mpn) = LOWER(?1) ORDER BY stock DESC")
                .unwrap();
            let rows = stmt
                .query_map([&variant], |row| Ok(row_to_dict(row, subcategories)))
                .unwrap();
            for part in rows.filter_map(|r| r.ok()) {
                let lcsc = part["lcsc"].as_str().unwrap_or_default().to_string();
                if !seen_lcsc.contains(&lcsc) {
                    seen_lcsc.insert(lcsc);
                    results.push(part);
                }
            }
            if !results.is_empty() {
                break;
            }
        }
    }

    // 3. Fall back to FTS if no exact matches
    if results.is_empty() {
        for variant in normalize_mpn(mpn) {
            let escaped = variant.replace('"', "\"\"");
            let fts_query = format!("\"{escaped}\"*");
            let mut stmt = conn
                .prepare(
                    "SELECT c.* FROM components c \
                     JOIN components_fts f ON c.lcsc = f.lcsc \
                     WHERE f.components_fts MATCH ?1 \
                     ORDER BY c.stock DESC LIMIT 10",
                )
                .unwrap();
            let rows = stmt
                .query_map([&fts_query], |row| Ok(row_to_dict(row, subcategories)))
                .unwrap();
            for part in rows.filter_map(|r| r.ok()) {
                let lcsc = part["lcsc"].as_str().unwrap_or_default().to_string();
                if !seen_lcsc.contains(&lcsc) {
                    seen_lcsc.insert(lcsc);
                    results.push(part);
                }
            }
            if !results.is_empty() {
                break;
            }
        }
    }

    results
}

pub fn get_by_lcsc(conn: &Connection, lcsc: &str, subcategories: &BTreeMap<i64, SubcategoryInfo>) -> Option<Value> {
    let mut stmt = conn.prepare("SELECT * FROM components WHERE lcsc = ?1").unwrap();
    let mut rows = stmt.query([lcsc.to_uppercase()]).unwrap();
    rows.next().unwrap().map(|row| row_to_dict(row, subcategories))
}

pub const MAX_BATCH_SIZE: usize = 1000;

pub fn get_by_lcsc_batch(
    conn: &Connection,
    lcsc_codes: &[String],
    subcategories: &BTreeMap<i64, SubcategoryInfo>,
) -> Result<HashMap<String, Option<Value>>, String> {
    if lcsc_codes.is_empty() {
        return Ok(HashMap::new());
    }
    if lcsc_codes.len() > MAX_BATCH_SIZE {
        return Err(format!(
            "Batch size {} exceeds maximum of {MAX_BATCH_SIZE}. Split into smaller batches.",
            lcsc_codes.len()
        ));
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut normalized: Vec<String> = Vec::new();
    for code in lcsc_codes {
        let upper = code.to_uppercase();
        if seen.insert(upper.clone()) {
            normalized.push(upper);
        }
    }

    let placeholders = vec!["?"; normalized.len()].join(",");
    let sql = format!("SELECT * FROM components WHERE lcsc IN ({placeholders})");
    let mut stmt = conn.prepare(&sql).unwrap();
    let params: Vec<&dyn rusqlite::ToSql> = normalized.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(params.as_slice(), |row| Ok(row_to_dict(row, subcategories))).unwrap();

    let mut results: HashMap<String, Option<Value>> = normalized.iter().map(|c| (c.clone(), None)).collect();
    for part in rows.filter_map(|r| r.ok()) {
        let lcsc = part["lcsc"].as_str().unwrap_or_default().to_string();
        results.insert(lcsc, Some(part));
    }

    Ok(results)
}
