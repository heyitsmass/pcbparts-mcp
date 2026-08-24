use crate::mpn::{looks_like_mpn, normalize_mpn};
use crate::query_builder::{
    build_fts_clause, build_library_type_clause, build_manufacturer_clause, build_mounting_type_clause,
    build_package_clause, build_sort_clause, build_spec_filter_clauses, build_subcategory_clause,
    needs_numeric_post_filter, SqlParam,
};
use crate::resolvers::{expand_package, resolve_manufacturer};
use crate::result::{row_to_dict, SubcategoryInfo};
use crate::spec_filter::SpecFilter;
use pcbparts_parsers::alternatives::dimension_spec_fields;
use pcbparts_parsers::parsers::parse_dimensions_from_package;
use pcbparts_parsers::subcategory_aliases::{find_similar_subcategories, resolve_subcategory_name, SimilarSubcategory};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

pub struct CategoryInfo {
    pub name: String,
}

pub struct SearchEngine {
    subcategories: BTreeMap<i64, SubcategoryInfo>,
    categories: BTreeMap<i64, CategoryInfo>,
    subcategory_name_to_id: HashMap<String, i64>,
    category_name_to_id: HashMap<String, i64>,
    category_to_subcategories: BTreeMap<i64, Vec<i64>>,
}

pub struct SearchParams {
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

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            query: None,
            subcategory_id: None,
            subcategory_name: None,
            category_id: None,
            category_name: None,
            spec_filters: Vec::new(),
            library_type: None,
            prefer_no_fee: false,
            min_stock: 0,
            package: None,
            packages: None,
            manufacturer: None,
            mounting_type: None,
            match_all_terms: false,
            sort_by: "relevance".to_string(),
            limit: 50,
            offset: 0,
        }
    }
}

fn empty_counts() -> Value {
    json!({"basic": 0, "preferred": 0, "extended": 0})
}

impl SearchEngine {
    pub fn new(
        subcategories: BTreeMap<i64, SubcategoryInfo>,
        categories: BTreeMap<i64, CategoryInfo>,
        subcategory_name_to_id: HashMap<String, i64>,
        category_name_to_id: HashMap<String, i64>,
        category_to_subcategories: BTreeMap<i64, Vec<i64>>,
    ) -> Self {
        Self { subcategories, categories, subcategory_name_to_id, category_name_to_id, category_to_subcategories }
    }

    /// Resolve subcategory name to ID (delegates to Phase 2A's already-fixed,
    /// deterministic implementation — no logic duplicated here).
    pub fn resolve_subcategory_name(&self, name: &str) -> Option<i64> {
        resolve_subcategory_name(name, &self.subcategory_name_to_id, None)
    }

    /// Resolve category name to ID. Case-insensitive, supports partial match
    /// (exact match first, then shortest-containing match with a
    /// deterministic (length, key) tie-break — see Task 6's Design Decisions).
    pub fn resolve_category_name(&self, name: &str) -> Option<i64> {
        let name_lower = name.to_lowercase();
        if let Some(&id) = self.category_name_to_id.get(&name_lower) {
            return Some(id);
        }
        let mut matches: Vec<(&str, i64)> = self
            .category_name_to_id
            .iter()
            .filter(|(k, _)| k.contains(&name_lower))
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        if matches.is_empty() {
            return None;
        }
        matches.sort_by_key(|(k, _)| (k.len(), *k));
        Some(matches[0].1)
    }

    fn find_similar_subcategories(&self, name: &str, limit: usize) -> Vec<SimilarSubcategory> {
        let info: HashMap<i64, (String, String)> = self
            .subcategories
            .iter()
            .map(|(id, i)| (*id, (i.name.clone(), i.category_name.clone().unwrap_or_default())))
            .collect();
        find_similar_subcategories(name, &self.subcategory_name_to_id, &info, limit)
    }

    fn subcategory_to_category_id(&self) -> BTreeMap<i64, i64> {
        self.subcategories.iter().map(|(id, info)| (*id, info.category_id)).collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_search(
        &self,
        conn: &Connection,
        query: Option<&str>,
        subcategory_id: Option<i64>,
        category_id: Option<i64>,
        spec_filters: &[SpecFilter],
        library_type: Option<&str>,
        min_stock: i64,
        expanded_packages: &[String],
        manufacturer: Option<&str>,
        mounting_type: Option<&str>,
        match_all_terms: bool,
        sort_by: &str,
        prefer_no_fee: bool,
        limit: i64,
        offset: i64,
    ) -> Value {
        let mut sql_parts = vec!["SELECT * FROM components WHERE 1=1".to_string()];
        let mut count_parts = vec!["SELECT COUNT(*) FROM components WHERE 1=1".to_string()];
        let mut params: Vec<SqlParam> = Vec::new();
        let mut count_params: Vec<SqlParam> = Vec::new();

        if let Some(q) = query {
            let (fts_sql, fts_params) = build_fts_clause(q, match_all_terms);
            if !fts_sql.is_empty() {
                sql_parts.push(fts_sql.clone());
                count_parts.push(fts_sql);
                for p in fts_params {
                    params.push(SqlParam::Text(p.clone()));
                    count_params.push(SqlParam::Text(p));
                }
            }
        }

        let (subcat_sql, subcat_params) =
            build_subcategory_clause(subcategory_id, category_id, &self.subcategory_to_category_id(), Some(&self.category_to_subcategories));
        if !subcat_sql.is_empty() {
            sql_parts.push(subcat_sql.clone());
            count_parts.push(subcat_sql);
            for p in &subcat_params {
                params.push(SqlParam::Integer(*p));
                count_params.push(SqlParam::Integer(*p));
            }
        }

        let lib_type_sql = build_library_type_clause(library_type);
        if !lib_type_sql.is_empty() {
            sql_parts.push(lib_type_sql.clone());
            count_parts.push(lib_type_sql);
        }

        let (stock_sql, stock_params) = crate::query_builder::build_stock_clause(min_stock);
        if !stock_sql.is_empty() {
            sql_parts.push(stock_sql.clone());
            count_parts.push(stock_sql);
            for p in &stock_params {
                params.push(SqlParam::Integer(*p));
                count_params.push(SqlParam::Integer(*p));
            }
        }

        if !expanded_packages.is_empty() {
            let (pkg_sql, pkg_params) = build_package_clause(expanded_packages);
            sql_parts.push(pkg_sql.clone());
            count_parts.push(pkg_sql);
            for p in pkg_params {
                params.push(SqlParam::Text(p.clone()));
                count_params.push(SqlParam::Text(p));
            }
        }

        if let Some(m) = manufacturer {
            let resolved = resolve_manufacturer(m);
            let (mfr_sql, mfr_params) = build_manufacturer_clause(&resolved);
            if !mfr_sql.is_empty() {
                sql_parts.push(mfr_sql.clone());
                count_parts.push(mfr_sql);
                for p in mfr_params {
                    params.push(SqlParam::Text(p.clone()));
                    count_params.push(SqlParam::Text(p));
                }
            }
        }

        if let Some(mt) = mounting_type {
            let (mount_sql, mount_params) = build_mounting_type_clause(Some(mt));
            if !mount_sql.is_empty() {
                sql_parts.push(mount_sql.clone());
                count_parts.push(mount_sql);
                for p in mount_params {
                    params.push(SqlParam::Text(p.clone()));
                    count_params.push(SqlParam::Text(p));
                }
            }
        }

        let (spec_sqls, spec_params, post_filter_metadata) = build_spec_filter_clauses(spec_filters);
        for s in &spec_sqls {
            sql_parts.push(s.clone());
            count_parts.push(s.clone());
        }
        for p in spec_params {
            params.push(p.clone());
            count_params.push(p);
        }

        sql_parts.push(build_sort_clause(sort_by, prefer_no_fee, query.is_some()));

        let has_numeric_filters = spec_filters.iter().any(needs_numeric_post_filter);
        let fetch_limit = (if has_numeric_filters { limit * 10 } else { limit }).min(500);

        sql_parts.push("LIMIT ? OFFSET ?".to_string());
        params.push(SqlParam::Integer(fetch_limit));
        params.push(SqlParam::Integer(offset));

        let sql = sql_parts.join(" ");
        let count_sql = count_parts.join(" ");

        let lib_count_sql = count_sql.replace("SELECT COUNT(*)", "SELECT library_type, COUNT(*)");
        let lib_count_sql = ["AND library_type = 'b'", "AND library_type = 'p'", "AND library_type = 'e'"]
            .iter()
            .fold(lib_count_sql, |acc, pattern| acc.replace(pattern, ""))
            + " GROUP BY library_type";

        let to_sql_params: Vec<Box<dyn rusqlite::ToSql>> = params
            .iter()
            .map(|p| -> Box<dyn rusqlite::ToSql> {
                match p {
                    SqlParam::Text(s) => Box::new(s.clone()),
                    SqlParam::Real(f) => Box::new(*f),
                    SqlParam::Integer(i) => Box::new(*i),
                }
            })
            .collect();
        let count_to_sql_params: Vec<Box<dyn rusqlite::ToSql>> = count_params
            .iter()
            .map(|p| -> Box<dyn rusqlite::ToSql> {
                match p {
                    SqlParam::Text(s) => Box::new(s.clone()),
                    SqlParam::Real(f) => Box::new(*f),
                    SqlParam::Integer(i) => Box::new(*i),
                }
            })
            .collect();
        let param_refs: Vec<&dyn rusqlite::ToSql> = to_sql_params.iter().map(|b| b.as_ref()).collect();
        let count_param_refs: Vec<&dyn rusqlite::ToSql> = count_to_sql_params.iter().map(|b| b.as_ref()).collect();

        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => {
                return json!({
                    "error": "Search failed: query too complex. Reduce the number of filters.",
                    "results": [], "total": 0, "library_type_counts": empty_counts(), "no_fee_available": false,
                });
            }
        };
        let rows: Vec<Value> = match stmt.query_map(param_refs.as_slice(), |row| Ok(row_to_dict(row, &self.subcategories))) {
            Ok(mapped) => mapped.filter_map(|r| r.ok()).collect(),
            Err(_) => {
                return json!({
                    "error": "Search failed: query too complex. Reduce the number of filters.",
                    "results": [], "total": 0, "library_type_counts": empty_counts(), "no_fee_available": false,
                });
            }
        };

        let mut lib_stmt = conn.prepare(&lib_count_sql).unwrap();
        let lib_rows: Vec<(String, i64)> = lib_stmt
            .query_map(count_param_refs.as_slice(), |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        let mut library_type_counts = HashMap::from([("basic", 0i64), ("preferred", 0i64), ("extended", 0i64)]);
        let mut total = 0i64;
        for (code, count) in lib_rows {
            let name = match code.as_str() { "b" => "basic", "p" => "preferred", "e" => "extended", _ => "" };
            if let Some(v) = library_type_counts.get_mut(name) {
                *v = count;
            }
            total += count;
        }

        let mut results = Vec::new();
        for part in rows {
            if !post_filter_metadata.is_empty() {
                let mut passes = true;
                let empty = json!({});
                let part_specs = part.get("specs").unwrap_or(&empty).as_object().cloned().unwrap_or_default();

                for meta in &post_filter_metadata {
                    let Some(target_value) = meta.target_value else { continue };
                    let Some(parser) = meta.parser else { continue };

                    let mut part_value: Option<f64> = None;
                    for (attr_name, attr_value) in &part_specs {
                        if meta.attr_names.contains(attr_name) {
                            if let Some(v) = attr_value.as_str().and_then(parser) {
                                part_value = Some(v);
                                break;
                            }
                        }
                    }

                    if part_value.is_none() && dimension_spec_fields().contains(meta.spec_filter.name.as_str()) {
                        let pkg = part.get("package").and_then(|v| v.as_str()).unwrap_or("");
                        let (diameter_mm, height_mm) = parse_dimensions_from_package(pkg);
                        part_value = if meta.spec_filter.name == "Diameter" { diameter_mm } else { height_mm };
                    }

                    let Some(part_value) = part_value else { passes = false; break };

                    let epsilon = if target_value != 0.0 { target_value.abs() * 1e-9 } else { 1e-15 };
                    let is_frequency = meta.attr_names.iter().any(|n| n.to_lowercase().contains("frequency"));
                    let eq_epsilon = if is_frequency {
                        if target_value != 0.0 { target_value.abs() * 0.05 } else { 1e-9 }
                    } else if target_value != 0.0 {
                        target_value.abs() * 0.01
                    } else {
                        1e-9
                    };

                    use crate::spec_filter::SpecOperator::*;
                    let ok = match meta.spec_filter.operator {
                        Eq => (part_value - target_value).abs() <= eq_epsilon,
                        Ge => part_value >= target_value - epsilon,
                        Le => part_value <= target_value + epsilon,
                        Gt => part_value > target_value + epsilon,
                        Lt => part_value < target_value - epsilon,
                    };
                    if !ok {
                        passes = false;
                        break;
                    }
                }
                if !passes {
                    continue;
                }
            }
            results.push(part);
            if results.len() as i64 >= limit {
                break;
            }
        }

        let no_fee_available = library_type_counts["basic"] > 0 || library_type_counts["preferred"] > 0;

        json!({
            "results": results,
            "total": total,
            "library_type_counts": {"basic": library_type_counts["basic"], "preferred": library_type_counts["preferred"], "extended": library_type_counts["extended"]},
            "no_fee_available": no_fee_available,
        })
    }

    pub fn search(&self, conn: &Connection, params: SearchParams) -> Value {
        let SearchParams {
            query, subcategory_id, subcategory_name, category_id, category_name, spec_filters,
            library_type, prefer_no_fee, min_stock, package, packages, manufacturer, mounting_type,
            match_all_terms, sort_by, limit, offset,
        } = params;

        let query = query.map(|q| crate::resolvers::expand_query_synonyms(&q));

        let mut resolved_subcategory_id = subcategory_id;
        let mut resolved_subcategory_display_name: Option<String> = None;
        if let Some(name) = &subcategory_name {
            if subcategory_id.is_none() {
                resolved_subcategory_id = self.resolve_subcategory_name(name);
                let Some(rid) = resolved_subcategory_id else {
                    let similar = self.find_similar_subcategories(name, 5);
                    return json!({
                        "error": format!("Subcategory not found: '{name}'"),
                        "hint": "Use list_categories and get_subcategories to see available options",
                        "similar_subcategories": similar.iter().map(|s| json!({"id": s.id, "name": s.name, "category": s.category})).collect::<Vec<_>>(),
                        "results": [], "total": 0, "library_type_counts": empty_counts(), "no_fee_available": false,
                    });
                };
                resolved_subcategory_display_name = self.subcategories.get(&rid).map(|i| i.name.clone());
            }
        }

        let mut resolved_category_id = category_id;
        let mut resolved_category_display_name: Option<String> = None;
        if let Some(name) = &category_name {
            if category_id.is_none() {
                resolved_category_id = self.resolve_category_name(name);
                let Some(rid) = resolved_category_id else {
                    return json!({
                        "error": format!("Category not found: '{name}'"),
                        "hint": "Use list_categories to see available categories",
                        "results": [], "total": 0, "library_type_counts": empty_counts(), "no_fee_available": false,
                    });
                };
                resolved_category_display_name = self.categories.get(&rid).map(|c| c.name.clone());
            }
        }

        if let Some(q) = &query {
            if q.chars().count() > 500 {
                return json!({"error": "Query too long (max 500 characters)", "results": [], "total": 0, "library_type_counts": empty_counts(), "no_fee_available": false});
            }
            if q.chars().any(|c| (c as u32) < 32 && !['\t', '\n', '\r'].contains(&c)) || q.contains('\0') {
                return json!({"error": "Query contains invalid characters", "results": [], "total": 0, "library_type_counts": empty_counts(), "no_fee_available": false});
            }
            let (fts_sql, _) = build_fts_clause(q, match_all_terms);
            if fts_sql.is_empty() {
                return json!({"error": "Query contains no searchable terms", "results": [], "total": 0, "library_type_counts": empty_counts(), "no_fee_available": false});
            }
        }

        let mut expanded_packages: Vec<String> = Vec::new();
        if let Some(pkgs) = &packages {
            for p in pkgs {
                expanded_packages.extend(expand_package(p));
            }
        } else if let Some(p) = &package {
            expanded_packages = expand_package(p);
        }

        let mut search_result = self.execute_search(
            conn, query.as_deref(), resolved_subcategory_id, resolved_category_id, &spec_filters,
            library_type.as_deref(), min_stock, &expanded_packages, manufacturer.as_deref(),
            mounting_type.as_deref(), match_all_terms, &sort_by, prefer_no_fee, limit, offset,
        );

        let mut mpn_retry_query: Option<String> = None;
        if search_result["total"] == 0 {
            if let Some(q) = &query {
                if looks_like_mpn(q) {
                    for variant in normalize_mpn(q).into_iter().skip(1) {
                        let retry = self.execute_search(
                            conn, Some(&variant), resolved_subcategory_id, resolved_category_id, &spec_filters,
                            library_type.as_deref(), min_stock, &expanded_packages, manufacturer.as_deref(),
                            mounting_type.as_deref(), match_all_terms, &sort_by, prefer_no_fee, limit, offset,
                        );
                        if retry["total"].as_i64().unwrap_or(0) > 0 {
                            mpn_retry_query = Some(variant);
                            search_result = retry;
                            break;
                        }
                    }
                }
            }
        }

        let results = search_result["results"].clone();
        let returned = results.as_array().map(|a| a.len()).unwrap_or(0);

        let mut response = json!({
            "results": results,
            "total": search_result["total"],
            "page_info": {"limit": limit, "offset": offset, "returned": returned},
            "filters_applied": {
                "query": query,
                "subcategory_id": resolved_subcategory_id,
                "subcategory_name": subcategory_name,
                "subcategory_resolved": resolved_subcategory_display_name,
                "category_id": resolved_category_id,
                "category_name": category_name,
                "category_resolved": resolved_category_display_name,
                "spec_filters": spec_filters.iter().map(|f| f.to_dict()).collect::<Vec<_>>(),
                "library_type": library_type,
                "prefer_no_fee": prefer_no_fee,
                "min_stock": min_stock,
                "package": package,
                "packages": packages,
                "manufacturer": manufacturer,
                "match_all_terms": match_all_terms,
            },
            "library_type_counts": search_result["library_type_counts"],
            "no_fee_available": search_result["no_fee_available"],
        });

        if let Some(variant) = mpn_retry_query {
            response["mpn_normalized"] = json!({
                "original_query": query,
                "matched_query": variant,
                "note": "Original query had no results; found matches using normalized MPN variant",
            });
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::collections::{BTreeMap, HashMap};

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE components (
                lcsc TEXT, mpn TEXT, manufacturer TEXT, package TEXT, stock INTEGER,
                library_type TEXT, subcategory_id INTEGER, price REAL, description TEXT, attributes TEXT,
                resistance_ohms REAL, voltage_max_v REAL
            );
            CREATE VIRTUAL TABLE components_fts USING fts5(lcsc UNINDEXED, mpn, manufacturer, description, tokenize='porter unicode61');
            INSERT INTO components VALUES
                ('C25804', '0603WAF1002T5E', 'UNI-ROYAL', '0603', 15959990, 'b', 2980, 0.0067,
                 '10kOhm resistor 0603', '[[\"Resistance\", \"10k\"]]', 10000.0, NULL),
                ('C8734', 'STM32F103C8T6', 'STMicroelectronics', 'LQFP-48', 251395, 'p', 2584, 1.7199,
                 'STM32F103C8T6 microcontroller', '[]', NULL, NULL);
            INSERT INTO components_fts (rowid, lcsc, mpn, manufacturer, description)
                SELECT rowid, lcsc, mpn, manufacturer, description FROM components;"
        ).unwrap();
        conn
    }

    fn test_engine() -> SearchEngine {
        SearchEngine::new(
            BTreeMap::from([
                (2980, SubcategoryInfo { name: "Chip Resistor - Surface Mount".to_string(), category_id: 10, category_name: Some("Resistors".to_string()) }),
                (2584, SubcategoryInfo { name: "Microcontrollers (MCU/MPU/SOC)".to_string(), category_id: 30, category_name: Some("Embedded Processors & Controllers".to_string()) }),
            ]),
            BTreeMap::from([
                (10, CategoryInfo { name: "Resistors".to_string() }),
                (30, CategoryInfo { name: "Embedded Processors & Controllers".to_string() }),
            ]),
            HashMap::from([
                ("chip resistor - surface mount".to_string(), 2980i64),
                ("microcontrollers (mcu/mpu/soc)".to_string(), 2584i64),
            ]),
            HashMap::from([
                ("resistors".to_string(), 10i64),
                ("embedded processors & controllers".to_string(), 30i64),
            ]),
            BTreeMap::from([(10, vec![2980i64]), (30, vec![2584i64])]),
        )
    }

    #[test]
    fn test_plain_fts_query() {
        let conn = test_conn();
        let engine = test_engine();
        let result = engine.search(&conn, SearchParams { query: Some("resistor".to_string()), min_stock: 10, ..Default::default() });
        assert!(result["total"].as_i64().unwrap() >= 1);
        assert_eq!(result["results"][0]["lcsc"], "C25804");
    }

    #[test]
    fn test_subcategory_by_id() {
        let conn = test_conn();
        let engine = test_engine();
        let result = engine.search(&conn, SearchParams { subcategory_id: Some(2980), min_stock: 0, ..Default::default() });
        assert_eq!(result["results"][0]["subcategory_id"], 2980);
    }

    #[test]
    fn test_spec_filter_numeric_column() {
        let conn = test_conn();
        let engine = test_engine();
        let filters = vec![crate::spec_filter::SpecFilter::new("Resistance", "=", "10k").unwrap()];
        let result = engine.search(&conn, SearchParams { spec_filters: filters, min_stock: 0, ..Default::default() });
        assert_eq!(result["results"][0]["lcsc"], "C25804");
    }

    #[test]
    fn test_subcategory_not_found_returns_error_with_suggestions() {
        let conn = test_conn();
        let engine = test_engine();
        let result = engine.search(&conn, SearchParams { subcategory_name: Some("bogus-xyz".to_string()), ..Default::default() });
        assert!(result["error"].as_str().unwrap().contains("Subcategory not found"));
        assert_eq!(result["total"], 0);
    }

    #[test]
    fn test_category_not_found_returns_error() {
        let conn = test_conn();
        let engine = test_engine();
        let result = engine.search(&conn, SearchParams { category_name: Some("bogus-xyz".to_string()), ..Default::default() });
        assert!(result["error"].as_str().unwrap().contains("Category not found"));
    }

    #[test]
    fn test_zero_results() {
        let conn = test_conn();
        let engine = test_engine();
        let result = engine.search(&conn, SearchParams { query: Some("zzzznonexistentxyz".to_string()), min_stock: 0, ..Default::default() });
        assert_eq!(result["total"], 0);
        assert_eq!(result["results"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_mpn_retry_path() {
        let conn = test_conn();
        let engine = test_engine();
        let result = engine.search(&conn, SearchParams { query: Some("STM32F103C8T6-TR".to_string()), min_stock: 0, ..Default::default() });
        assert_eq!(result["total"], 1);
        assert_eq!(result["mpn_normalized"]["matched_query"], "STM32F103C8T6");
        assert_eq!(result["results"][0]["lcsc"], "C8734");
    }

    #[test]
    fn test_resolve_category_name_shortest_match() {
        let engine = test_engine();
        assert_eq!(engine.resolve_category_name("resistor"), Some(10));
    }
}
