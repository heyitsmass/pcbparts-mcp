//! Category and subcategory functions — ported 1:1 from `db/categories.py`.
use pcbparts_parsers::alternatives::{spec_parsers, SpecParser};
use pcbparts_search::result::{row_to_dict, SubcategoryInfo};
use pcbparts_search::spec_filter::escape_like;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn get_subcategory_name(subcategory_id: i64, subcategories: &BTreeMap<i64, SubcategoryInfo>) -> Option<String> {
    subcategories.get(&subcategory_id).map(|s| s.name.clone())
}

pub fn get_category_for_subcategory(
    subcategory_id: i64,
    subcategories: &BTreeMap<i64, SubcategoryInfo>,
) -> Option<(i64, Option<String>)> {
    subcategories.get(&subcategory_id).map(|s| (s.category_id, s.category_name.clone()))
}

pub fn get_categories_for_client(conn: &Connection, subcategories: &BTreeMap<i64, SubcategoryInfo>) -> Vec<Value> {
    let mut subcat_counts: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT subcategory_id, COUNT(*) FROM components GROUP BY subcategory_id").unwrap();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let id: i64 = row.get(0).unwrap();
            let count: i64 = row.get(1).unwrap();
            subcat_counts.insert(id, count);
        }
    }

    let mut categories_map: std::collections::BTreeMap<i64, (String, i64, Vec<Value>)> = std::collections::BTreeMap::new();
    for (subcat_id, info) in subcategories {
        let count = *subcat_counts.get(subcat_id).unwrap_or(&0);
        if count == 0 {
            continue;
        }
        let entry = categories_map
            .entry(info.category_id)
            .or_insert_with(|| (info.category_name.clone().unwrap_or_default(), 0, Vec::new()));
        entry.2.push(json!({"id": subcat_id, "name": info.name, "count": count}));
        entry.1 += count;
    }

    categories_map
        .into_iter()
        .map(|(cat_id, (name, count, subcats))| json!({"id": cat_id, "name": name, "count": count, "subcategories": subcats}))
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn find_by_subcategory(
    conn: &Connection,
    subcategories: &BTreeMap<i64, SubcategoryInfo>,
    subcategory_id: i64,
    primary_spec: Option<&str>,
    primary_value: Option<&str>,
    min_stock: i64,
    library_type: Option<&str>,
    prefer_no_fee: bool,
    limit: i64,
) -> Vec<Value> {
    let mut sql_parts = vec!["SELECT * FROM components WHERE subcategory_id = ?".to_string()];
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(subcategory_id)];

    if min_stock > 0 {
        sql_parts.push("AND stock >= ?".to_string());
        params.push(Box::new(min_stock));
    }

    if let Some(lt) = library_type {
        match lt {
            "basic" => sql_parts.push("AND library_type = 'b'".to_string()),
            "preferred" => sql_parts.push("AND library_type = 'p'".to_string()),
            "extended" => sql_parts.push("AND library_type = 'e'".to_string()),
            "no_fee" => sql_parts.push("AND library_type IN ('b', 'p')".to_string()),
            _ => {}
        }
    }

    let is_numeric_spec = primary_spec
        .map(|s| matches!(spec_parsers().get(s), Some(SpecParser::Parser(_))))
        .unwrap_or(false);
    if let (Some(spec), Some(value)) = (primary_spec, primary_value) {
        if !is_numeric_spec {
            sql_parts.push("AND attributes LIKE ? ESCAPE '\\'".to_string());
            let pattern = format!("%\"{}\", \"{}%", escape_like(spec), escape_like(value));
            params.push(Box::new(pattern));
        }
    }

    if prefer_no_fee {
        sql_parts.push("ORDER BY CASE library_type WHEN 'b' THEN 1 WHEN 'p' THEN 2 ELSE 3 END, stock DESC".to_string());
    } else {
        sql_parts.push("ORDER BY stock DESC".to_string());
    }
    sql_parts.push("LIMIT ?".to_string());
    params.push(Box::new(limit * 2));

    let sql = sql_parts.join(" ");
    let mut stmt = conn.prepare(&sql).unwrap();
    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), |row| Ok(row_to_dict(row, subcategories))).unwrap();

    let mut results = Vec::new();
    for part in rows.filter_map(|r| r.ok()) {
        if let (Some(spec), Some(value)) = (primary_spec, primary_value) {
            if is_numeric_spec {
                if let Some(SpecParser::Parser(parser)) = spec_parsers().get(spec) {
                    if let Some(target) = parser(value) {
                        let part_value = part.get("specs").and_then(|s| s.get(spec)).and_then(|v| v.as_str());
                        match part_value {
                            Some(pv) => match parser(pv) {
                                None => continue,
                                Some(parsed) => {
                                    if target == 0.0 {
                                        if parsed != 0.0 {
                                            continue;
                                        }
                                    } else if ((parsed - target) / target).abs() > 0.02 {
                                        continue;
                                    }
                                }
                            },
                            None => continue,
                        }
                    }
                }
            }
        }
        results.push(part);
        if results.len() as i64 >= limit {
            break;
        }
    }

    results
}
