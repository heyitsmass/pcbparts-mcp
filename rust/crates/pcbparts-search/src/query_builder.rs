use crate::spec_filter::{escape_like, generate_value_patterns, get_attribute_names, spec_to_column, SpecFilter, SpecOperator};
use pcbparts_parsers::alternatives::{spec_parsers, SpecParser};
use regex::Regex;
use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq)]
pub enum SqlParam {
    Text(String),
    Real(f64),
    Integer(i64),
}

pub struct PostFilterMeta {
    pub spec_filter: SpecFilter,
    pub attr_names: HashSet<String>,
    pub parser: Option<crate::spec_filter::SpecParserFn>,
    pub target_value: Option<f64>,
}

pub enum GroupedFilter {
    Single(SpecFilter),
    Grouped(String, Vec<String>),
}

static CONTROL_CHAR_OK: [char; 3] = ['\t', '\n', '\r'];

/// Build FTS (full-text search) WHERE clause.
pub fn build_fts_clause(query: &str, match_all_terms: bool) -> (String, Vec<String>) {
    if query.chars().count() > 500 {
        return (String::new(), vec![]);
    }
    if query.chars().any(|c| (c as u32) < 32 && !CONTROL_CHAR_OK.contains(&c)) || query.contains('\0') {
        return (String::new(), vec![]);
    }

    let fts_parts: Vec<String> = query
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|token| format!("\"{}\"*", token.replace('"', "\"\"")))
        .collect();

    if fts_parts.is_empty() {
        return (String::new(), vec![]);
    }

    let fts_query = if match_all_terms { fts_parts.join(" ") } else { fts_parts.join(" OR ") };
    let sql = "\n        AND lcsc IN (\n            SELECT lcsc FROM components_fts\n            WHERE components_fts MATCH ?\n        )\n    ".to_string();
    (sql, vec![fts_query])
}

/// Build subcategory/category filter clause.
///
/// `subcategories` maps subcategory_id -> category_id (the only field this
/// function needs from `engine.rs`'s richer `SubcategoryInfo`).
pub fn build_subcategory_clause(
    subcategory_id: Option<i64>,
    category_id: Option<i64>,
    subcategories: &BTreeMap<i64, i64>,
    category_to_subcategories: Option<&BTreeMap<i64, Vec<i64>>>,
) -> (String, Vec<i64>) {
    if let Some(sid) = subcategory_id {
        if sid != 0 {
            return ("AND subcategory_id = ?".to_string(), vec![sid]);
        }
    }
    if let Some(cid) = category_id {
        if cid != 0 {
            let subcat_ids: Vec<i64> = match category_to_subcategories.and_then(|m| m.get(&cid)) {
                Some(ids) => ids.clone(),
                None => subcategories.iter().filter(|(_, &c)| c == cid).map(|(&sid, _)| sid).collect(),
            };
            if !subcat_ids.is_empty() {
                let placeholders = vec!["?"; subcat_ids.len()].join(",");
                return (format!("AND subcategory_id IN ({placeholders})"), subcat_ids);
            }
        }
    }
    (String::new(), vec![])
}

/// Build library type filter clause (no params needed).
pub fn build_library_type_clause(library_type: Option<&str>) -> String {
    match library_type {
        Some("basic") => "AND library_type = 'b'".to_string(),
        Some("preferred") => "AND library_type = 'p'".to_string(),
        Some("extended") => "AND library_type = 'e'".to_string(),
        Some("no_fee") => "AND library_type IN ('b', 'p')".to_string(),
        _ => String::new(),
    }
}

/// Build minimum stock filter clause.
pub fn build_stock_clause(min_stock: i64) -> (String, Vec<i64>) {
    if min_stock > 0 {
        ("AND stock >= ?".to_string(), vec![min_stock])
    } else {
        (String::new(), vec![])
    }
}

/// Expand a package name to include common JLCPCB variations (QFP/QFN/SOIC prefixes).
pub fn expand_package_aliases(pkg: &str) -> Vec<String> {
    let pkg_upper = pkg.to_uppercase();
    let mut variants = vec![pkg_upper.clone()];

    static QFP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([TLP])?QFP-?(\d+)(.*)$").unwrap());
    if let Some(caps) = QFP_RE.captures(&pkg_upper) {
        let prefix = caps.get(1).map(|m| m.as_str());
        let pins = &caps[2];
        let suffix = caps.get(3).map(|m| m.as_str()).unwrap_or("");
        if prefix.is_some() {
            let bare = format!("QFP-{pins}{suffix}");
            if !variants.contains(&bare) {
                variants.push(bare);
            }
        }
        for p in ["", "T", "L", "P", "H"] {
            let var = if p.is_empty() { format!("QFP-{pins}") } else { format!("{p}QFP-{pins}") };
            if !variants.contains(&var) {
                variants.push(var);
            }
        }
    }

    static QFN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([LWVTU])?QFN-?(\d+)(.*)$").unwrap());
    if let Some(caps) = QFN_RE.captures(&pkg_upper) {
        let pins = &caps[2];
        for p in ["", "L", "W", "V", "T", "U"] {
            let var = if p.is_empty() { format!("QFN-{pins}") } else { format!("{p}QFN-{pins}") };
            if !variants.contains(&var) {
                variants.push(var);
            }
        }
    }

    static SOIC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(SOIC|SO|SOP)-?(\d+)(.*)$").unwrap());
    if let Some(caps) = SOIC_RE.captures(&pkg_upper) {
        let pins = &caps[2];
        for p in ["SOIC-", "SOP-", "SO-"] {
            let var = format!("{p}{pins}");
            if !variants.contains(&var) {
                variants.push(var);
            }
        }
    }

    variants
}

/// Build package filter clause (prefix-matched, alias-expanded, OR'd).
pub fn build_package_clause(packages: &[String]) -> (String, Vec<String>) {
    if packages.is_empty() {
        return (String::new(), vec![]);
    }

    let mut expanded: Vec<String> = Vec::new();
    for pkg in packages {
        expanded.extend(expand_package_aliases(pkg));
    }
    let mut seen = HashSet::new();
    let unique: Vec<String> = expanded.into_iter().filter(|p| seen.insert(p.clone())).collect();

    let mut or_conditions = Vec::new();
    let mut params = Vec::new();
    for pkg in &unique {
        let escaped = pkg.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
        or_conditions.push("package LIKE ? ESCAPE '\\'".to_string());
        params.push(format!("{escaped}%"));
    }
    (format!("AND ({})", or_conditions.join(" OR ")), params)
}

/// Build manufacturer filter clause (manufacturer already resolved by the caller).
pub fn build_manufacturer_clause(manufacturer: &str) -> (String, Vec<String>) {
    if !manufacturer.is_empty() {
        ("AND LOWER(manufacturer) = LOWER(?)".to_string(), vec![manufacturer.to_string()])
    } else {
        (String::new(), vec![])
    }
}

/// Build mounting type filter clause (description-text based).
pub fn build_mounting_type_clause(mounting_type: Option<&str>) -> (String, Vec<String>) {
    let Some(mounting_type) = mounting_type else { return (String::new(), vec![]) };
    match mounting_type.to_lowercase().as_str() {
        "through hole" | "tht" | "through-hole" => (
            "AND (description LIKE ? OR description LIKE ?)".to_string(),
            vec!["%Through Hole%".to_string(), "%Plugin%".to_string()],
        ),
        "smd" | "surface mount" | "smt" => (
            "AND (description LIKE ? OR description LIKE ?)".to_string(),
            vec!["%Surface Mount%".to_string(), "%SMD%".to_string()],
        ),
        _ => (String::new(), vec![]),
    }
}

/// Group filters with the same (name, "=" operator) into OR groups (e.g. multi-value Interface).
pub fn group_multi_value_filters(spec_filters: &[SpecFilter]) -> Vec<GroupedFilter> {
    use std::collections::HashMap;

    let mut groups: HashMap<(String, &'static str), Vec<&SpecFilter>> = HashMap::new();
    for f in spec_filters {
        groups.entry((f.name.to_lowercase(), f.operator.as_str())).or_default().push(f);
    }

    let mut result = Vec::new();
    let mut processed: HashSet<(String, &'static str)> = HashSet::new();

    for spec_filter in spec_filters {
        let key = (spec_filter.name.to_lowercase(), spec_filter.operator.as_str());
        if processed.contains(&key) {
            continue;
        }
        let filters_in_group = &groups[&key];
        if spec_filter.operator == SpecOperator::Eq && filters_in_group.len() > 1 {
            let values: Vec<String> = filters_in_group.iter().map(|f| f.value.clone()).collect();
            result.push(GroupedFilter::Grouped(spec_filter.name.clone(), values));
        } else {
            result.push(GroupedFilter::Single(spec_filter.clone()));
        }
        processed.insert(key);
    }

    result
}

fn leading_number(value: &str) -> Option<String> {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+(?:\.\d+)?)").unwrap());
    RE.captures(value).map(|c| c[1].to_string())
}

fn equality_pattern(name: &str, value: &str, use_substring_match: bool, is_impedance_at_freq: bool) -> String {
    if use_substring_match {
        format!("%\"{}\"%{}%", escape_like(name), escape_like(value))
    } else if is_impedance_at_freq {
        match leading_number(value) {
            Some(numeric_part) => format!("%\"{}\", \"{numeric_part}%", escape_like(name)),
            None => format!("%\"{}\", \"{}%", escape_like(name), escape_like(value)),
        }
    } else {
        format!("%\"{}\", \"{}\"%", escape_like(name), escape_like(value))
    }
}

/// Build spec filter clauses for SQL, plus metadata for filters needing Python-side
/// (here: Rust-side) post-filtering.
pub fn build_spec_filter_clauses(spec_filters: &[SpecFilter]) -> (Vec<String>, Vec<SqlParam>, Vec<PostFilterMeta>) {
    let mut sql_clauses = Vec::new();
    let mut params: Vec<SqlParam> = Vec::new();
    let mut post_filter_metadata = Vec::new();

    let column_map = spec_to_column();
    let parsers = spec_parsers();

    for item in group_multi_value_filters(spec_filters) {
        match item {
            GroupedFilter::Grouped(spec_name, values) => {
                let attr_names = get_attribute_names(&spec_name);
                let use_substring_match = spec_name.to_lowercase() == "interface";
                let is_impedance_at_freq = spec_name.to_lowercase() == "impedance @ frequency";

                let mut or_conditions = Vec::new();
                for value in &values {
                    for name in &attr_names {
                        or_conditions.push("attributes LIKE ? ESCAPE '\\'".to_string());
                        params.push(SqlParam::Text(equality_pattern(name, value, use_substring_match, is_impedance_at_freq)));
                    }
                }
                if !or_conditions.is_empty() {
                    sql_clauses.push(format!("AND ({})", or_conditions.join(" OR ")));
                }
            }
            GroupedFilter::Single(spec_filter) => {
                let attr_names = get_attribute_names(&spec_filter.name);
                let mut candidate_names = vec![spec_filter.name.clone()];
                candidate_names.extend(attr_names.iter().cloned());

                let mut column_info = None;
                for name in &candidate_names {
                    if let Some(&(col, parser)) = column_map.get(name.as_str()) {
                        column_info = Some((col, parser));
                        break;
                    }
                }

                let mut handled = false;
                if let Some((column_name, mut parser)) = column_info {
                    if parser.is_none() {
                        for name in &attr_names {
                            match parsers.get(name.as_str()) {
                                Some(SpecParser::Parser(f)) => {
                                    parser = Some(*f);
                                    break;
                                }
                                Some(SpecParser::Special) => {
                                    panic!(
                                        "'{name}' spec parser is 'special' (non-callable) — Python's build_spec_filter_clauses crashes the same way on this input (SPEC_PARSERS[name] == \"special\" is truthy but not callable); preserved faithfully per this plan's Task 4 instruction, not fixed here"
                                    );
                                }
                                Some(SpecParser::StringMatch) => {}
                                None => {}
                            }
                        }
                    }
                    if let Some(parser_fn) = parser {
                        if let Some(parsed_value) = parser_fn(&spec_filter.value) {
                            match spec_filter.operator {
                                SpecOperator::Eq => {
                                    let tolerance = if parsed_value != 0.0 { parsed_value.abs() * 0.01 } else { 1e-9 };
                                    sql_clauses.push(format!("AND {column_name} BETWEEN ? AND ?"));
                                    params.push(SqlParam::Real(parsed_value - tolerance));
                                    params.push(SqlParam::Real(parsed_value + tolerance));
                                }
                                SpecOperator::Ge => { sql_clauses.push(format!("AND {column_name} >= ?")); params.push(SqlParam::Real(parsed_value)); }
                                SpecOperator::Le => { sql_clauses.push(format!("AND {column_name} <= ?")); params.push(SqlParam::Real(parsed_value)); }
                                SpecOperator::Gt => { sql_clauses.push(format!("AND {column_name} > ?")); params.push(SqlParam::Real(parsed_value)); }
                                SpecOperator::Lt => { sql_clauses.push(format!("AND {column_name} < ?")); params.push(SqlParam::Real(parsed_value)); }
                            }
                            handled = true;
                        }
                    }
                }

                if !handled {
                    let mut parser = None;
                    for name in &attr_names {
                        match parsers.get(name.as_str()) {
                            Some(SpecParser::Parser(f)) => {
                                parser = Some(*f);
                                break;
                            }
                            Some(SpecParser::Special) => {
                                panic!(
                                    "'{name}' spec parser is 'special' (non-callable) — Python's build_spec_filter_clauses crashes the same way on this input (SPEC_PARSERS[name] == \"special\" is truthy but not callable); preserved faithfully per this plan's Task 4 instruction, not fixed here"
                                );
                            }
                            Some(SpecParser::StringMatch) => {}
                            None => {}
                        }
                    }
                    let parsed_value = parser.and_then(|f| f(&spec_filter.value));

                    if let Some(parsed_value) = parsed_value {
                        if spec_filter.operator == SpecOperator::Eq {
                            let mut or_conditions = Vec::new();
                            for name in &attr_names {
                                for pattern in generate_value_patterns(name, &spec_filter.value, Some(parsed_value)) {
                                    or_conditions.push("attributes LIKE ? ESCAPE '\\'".to_string());
                                    params.push(SqlParam::Text(pattern));
                                }
                            }
                            if !or_conditions.is_empty() {
                                sql_clauses.push(format!("AND ({})", or_conditions.join(" OR ")));
                            }
                        } else {
                            let mut or_conditions = Vec::new();
                            for name in &attr_names {
                                or_conditions.push("attributes LIKE ? ESCAPE '\\'".to_string());
                                params.push(SqlParam::Text(format!("%\"{}\"%", escape_like(name))));
                            }
                            if !or_conditions.is_empty() {
                                sql_clauses.push(format!("AND ({})", or_conditions.join(" OR ")));
                            }
                        }

                        post_filter_metadata.push(PostFilterMeta {
                            spec_filter: spec_filter.clone(),
                            attr_names: attr_names.iter().cloned().collect(),
                            parser,
                            target_value: Some(parsed_value),
                        });
                    } else if spec_filter.operator == SpecOperator::Eq {
                        let use_substring_match = spec_filter.name.to_lowercase() == "interface";
                        let is_impedance_at_freq = spec_filter.name.to_lowercase() == "impedance @ frequency";
                        let mut or_conditions = Vec::new();
                        for name in &attr_names {
                            or_conditions.push("attributes LIKE ? ESCAPE '\\'".to_string());
                            params.push(SqlParam::Text(equality_pattern(name, &spec_filter.value, use_substring_match, is_impedance_at_freq)));
                        }
                        if !or_conditions.is_empty() {
                            sql_clauses.push(format!("AND ({})", or_conditions.join(" OR ")));
                        }
                    }
                }
            }
        }
    }

    (sql_clauses, params, post_filter_metadata)
}

/// Build ORDER BY clause.
pub fn build_sort_clause(sort_by: &str, prefer_no_fee: bool, has_query: bool) -> String {
    const LIB_TYPE_ORDER: &str = "CASE library_type WHEN 'b' THEN 1 WHEN 'p' THEN 2 ELSE 3 END";

    match sort_by {
        "price" => {
            if prefer_no_fee {
                format!("ORDER BY {LIB_TYPE_ORDER}, price ASC NULLS LAST")
            } else {
                "ORDER BY price ASC NULLS LAST".to_string()
            }
        }
        "relevance" if has_query => {
            if prefer_no_fee {
                format!("ORDER BY {LIB_TYPE_ORDER}, stock DESC")
            } else {
                "ORDER BY stock DESC".to_string()
            }
        }
        _ => {
            if prefer_no_fee {
                format!("ORDER BY {LIB_TYPE_ORDER}, stock DESC")
            } else {
                "ORDER BY stock DESC".to_string()
            }
        }
    }
}

/// Check if a spec filter needs post-filtering outside SQL.
pub fn needs_numeric_post_filter(spec_filter: &SpecFilter) -> bool {
    let attr_names = get_attribute_names(&spec_filter.name);
    let column_map = spec_to_column();

    let mut candidate_names = vec![spec_filter.name.clone()];
    candidate_names.extend(attr_names.iter().cloned());
    if candidate_names.iter().any(|n| column_map.contains_key(n.as_str())) {
        return false;
    }

    if matches!(spec_filter.operator, SpecOperator::Ge | SpecOperator::Le | SpecOperator::Gt | SpecOperator::Lt) {
        return true;
    }
    if spec_filter.operator == SpecOperator::Eq {
        let parsers = spec_parsers();
        for name in &attr_names {
            match parsers.get(name.as_str()) {
                Some(SpecParser::Parser(_)) => return true,
                // Python only checks SPEC_PARSERS.get(name) for truthiness, never calls the parser.
                // "special" is a truthy string, so Python returns True here (no crash).
                Some(SpecParser::Special) => return true,
                Some(SpecParser::StringMatch) => {}
                None => {}
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_build_fts_clause_single_term() {
        let (sql, params) = build_fts_clause("resistor", true);
        assert!(sql.contains("components_fts MATCH ?"));
        assert_eq!(params, vec!["\"resistor\"*"]);
    }

    #[test]
    fn test_build_fts_clause_multi_term_and_or() {
        let (_, params_and) = build_fts_clause("10k resistor", true);
        assert_eq!(params_and, vec!["\"10k\"* \"resistor\"*"]);
        let (_, params_or) = build_fts_clause("10k resistor", false);
        assert_eq!(params_or, vec!["\"10k\"* OR \"resistor\"*"]);
    }

    #[test]
    fn test_build_fts_clause_rejects_invalid() {
        assert_eq!(build_fts_clause("", true), (String::new(), vec![]));
        assert_eq!(build_fts_clause(&"x".repeat(501), true), (String::new(), vec![]));
        assert_eq!(build_fts_clause("bad\x00query", true), (String::new(), vec![]));
    }

    fn fake_subcategories() -> BTreeMap<i64, i64> {
        // maps subcategory_id -> category_id, matching what build_subcategory_clause needs
        BTreeMap::from([(1, 10), (2, 10), (3, 20)])
    }

    #[test]
    fn test_build_subcategory_clause_by_subcategory_id() {
        let (sql, params) = build_subcategory_clause(Some(1), None, &fake_subcategories(), None);
        assert_eq!(sql, "AND subcategory_id = ?");
        assert_eq!(params, vec![1]);
    }

    #[test]
    fn test_build_subcategory_clause_by_category_with_map() {
        let cat_to_subcat = BTreeMap::from([(10, vec![1, 2]), (20, vec![3])]);
        let (sql, params) = build_subcategory_clause(None, Some(10), &fake_subcategories(), Some(&cat_to_subcat));
        assert_eq!(sql, "AND subcategory_id IN (?,?)");
        assert_eq!(params, vec![1, 2]);
    }

    #[test]
    fn test_build_subcategory_clause_by_category_no_map() {
        let (sql, params) = build_subcategory_clause(None, Some(10), &fake_subcategories(), None);
        assert_eq!(sql, "AND subcategory_id IN (?,?)");
        assert_eq!(params, vec![1, 2]);
    }

    #[test]
    fn test_build_subcategory_clause_neither() {
        assert_eq!(build_subcategory_clause(None, None, &fake_subcategories(), None), (String::new(), vec![]));
    }

    #[test]
    fn test_build_library_type_clause() {
        assert_eq!(build_library_type_clause(Some("basic")), "AND library_type = 'b'");
        assert_eq!(build_library_type_clause(Some("preferred")), "AND library_type = 'p'");
        assert_eq!(build_library_type_clause(Some("extended")), "AND library_type = 'e'");
        assert_eq!(build_library_type_clause(Some("no_fee")), "AND library_type IN ('b', 'p')");
        assert_eq!(build_library_type_clause(None), "");
        assert_eq!(build_library_type_clause(Some("bogus")), "");
    }

    #[test]
    fn test_build_stock_clause() {
        assert_eq!(build_stock_clause(10), ("AND stock >= ?".to_string(), vec![10]));
        assert_eq!(build_stock_clause(0), (String::new(), vec![]));
        assert_eq!(build_stock_clause(-5), (String::new(), vec![]));
    }

    #[test]
    fn test_expand_package_aliases_qfp() {
        assert_eq!(
            expand_package_aliases("TQFP-44"),
            vec!["TQFP-44", "QFP-44", "LQFP-44", "PQFP-44", "HQFP-44"]
        );
    }

    #[test]
    fn test_expand_package_aliases_qfn() {
        assert_eq!(
            expand_package_aliases("QFN-56"),
            vec!["QFN-56", "LQFN-56", "WQFN-56", "VQFN-56", "TQFN-56", "UQFN-56"]
        );
    }

    #[test]
    fn test_expand_package_aliases_soic() {
        assert_eq!(expand_package_aliases("SOIC-8"), vec!["SOIC-8", "SOP-8", "SO-8"]);
    }

    #[test]
    fn test_expand_package_aliases_unmatched() {
        assert_eq!(expand_package_aliases("UFQFPN-48"), vec!["UFQFPN-48"]);
        assert_eq!(expand_package_aliases("SOT-23"), vec!["SOT-23"]);
    }

    #[test]
    fn test_build_package_clause_single() {
        let (sql, params) = build_package_clause(&["SOT-23".to_string()]);
        assert_eq!(sql, "AND (package LIKE ? ESCAPE '\\')");
        assert_eq!(params, vec!["SOT-23%"]);
    }

    #[test]
    fn test_build_package_clause_empty() {
        assert_eq!(build_package_clause(&[]), (String::new(), vec![]));
    }

    #[test]
    fn test_build_manufacturer_clause() {
        assert_eq!(
            build_manufacturer_clause("YAGEO"),
            ("AND LOWER(manufacturer) = LOWER(?)".to_string(), vec!["YAGEO".to_string()])
        );
        assert_eq!(build_manufacturer_clause(""), (String::new(), vec![]));
    }

    #[test]
    fn test_build_mounting_type_clause() {
        assert_eq!(
            build_mounting_type_clause(Some("Through Hole")),
            ("AND (description LIKE ? OR description LIKE ?)".to_string(), vec!["%Through Hole%".to_string(), "%Plugin%".to_string()])
        );
        assert_eq!(
            build_mounting_type_clause(Some("SMD")),
            ("AND (description LIKE ? OR description LIKE ?)".to_string(), vec!["%Surface Mount%".to_string(), "%SMD%".to_string()])
        );
        assert_eq!(build_mounting_type_clause(None), (String::new(), vec![]));
        assert_eq!(build_mounting_type_clause(Some("bogus")), (String::new(), vec![]));
    }

    #[test]
    fn test_group_multi_value_filters_groups_interface() {
        let filters = vec![
            SpecFilter::new("Interface", "=", "I2C").unwrap(),
            SpecFilter::new("Interface", "=", "SPI").unwrap(),
        ];
        let grouped = group_multi_value_filters(&filters);
        assert_eq!(grouped.len(), 1);
        match &grouped[0] {
            GroupedFilter::Grouped(name, values) => {
                assert_eq!(name, "Interface");
                assert_eq!(values, &vec!["I2C".to_string(), "SPI".to_string()]);
            }
            GroupedFilter::Single(_) => panic!("expected Grouped"),
        }
    }

    #[test]
    fn test_build_spec_filter_clauses_resistance_numeric_column() {
        let filters = vec![SpecFilter::new("Resistance", ">=", "10k").unwrap()];
        let (sql, params, meta) = build_spec_filter_clauses(&filters);
        assert_eq!(sql, vec!["AND resistance_ohms >= ?"]);
        assert_eq!(params, vec![SqlParam::Real(10000.0)]);
        assert_eq!(meta.len(), 0);
    }

    #[test]
    fn test_build_spec_filter_clauses_resistance_numeric_column_eq_tolerance() {
        let filters = vec![SpecFilter::new("Resistance", "=", "10k").unwrap()];
        let (sql, params, meta) = build_spec_filter_clauses(&filters);
        assert_eq!(sql, vec!["AND resistance_ohms BETWEEN ? AND ?"]);
        // 10k = 10000 ohms, tolerance = 10000 * 0.01 = 100
        assert_eq!(params, vec![SqlParam::Real(9900.0), SqlParam::Real(10100.0)]);
        assert_eq!(meta.len(), 0);
    }

    #[test]
    fn test_build_spec_filter_clauses_interface_grouped() {
        let filters = vec![
            SpecFilter::new("Interface", "=", "I2C").unwrap(),
            SpecFilter::new("Interface", "=", "SPI").unwrap(),
        ];
        let (sql, params, _) = build_spec_filter_clauses(&filters);
        assert_eq!(sql, vec!["AND (attributes LIKE ? ESCAPE '\\' OR attributes LIKE ? ESCAPE '\\')"]);
        assert_eq!(
            params,
            vec![SqlParam::Text("%\"Interface\"%I2C%".to_string()), SqlParam::Text("%\"Interface\"%SPI%".to_string())]
        );
    }

    #[test]
    fn test_build_spec_filter_clauses_string_exact() {
        let filters = vec![SpecFilter::new("Type", "=", "N-Channel").unwrap()];
        let (sql, params, _) = build_spec_filter_clauses(&filters);
        assert_eq!(sql, vec!["AND (attributes LIKE ? ESCAPE '\\')"]);
        assert_eq!(params, vec![SqlParam::Text("%\"Type\", \"N-Channel\"%".to_string())]);
    }

    #[test]
    fn test_build_sort_clause() {
        assert_eq!(
            build_sort_clause("price", true, false),
            "ORDER BY CASE library_type WHEN 'b' THEN 1 WHEN 'p' THEN 2 ELSE 3 END, price ASC NULLS LAST"
        );
        assert_eq!(build_sort_clause("price", false, false), "ORDER BY price ASC NULLS LAST");
        assert_eq!(
            build_sort_clause("relevance", true, true),
            "ORDER BY CASE library_type WHEN 'b' THEN 1 WHEN 'p' THEN 2 ELSE 3 END, stock DESC"
        );
        assert_eq!(build_sort_clause("stock", false, false), "ORDER BY stock DESC");
    }

    #[test]
    fn test_needs_numeric_post_filter() {
        assert!(!needs_numeric_post_filter(&SpecFilter::new("Resistance", ">=", "10k").unwrap()));
        assert!(!needs_numeric_post_filter(&SpecFilter::new("Type", "=", "N-Channel").unwrap()));
        assert!(needs_numeric_post_filter(&SpecFilter::new("Vgs(th)", "=", "2V").unwrap()));
    }

    #[test]
    fn test_needs_numeric_post_filter_impedance_at_frequency_special_parser() {
        // "Impedance @ Frequency" has SpecParser::Special (non-callable marker in Python).
        // Python only checks truthiness, never calls the parser, so it returns True.
        // This test verifies the Rust implementation matches Python's behavior.
        assert!(needs_numeric_post_filter(&SpecFilter::new("Impedance @ Frequency", "=", "50Ω").unwrap()));
    }
}
