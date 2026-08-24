//! Attribute discovery — ported 1:1 from `db/attributes.py`, with the `is_numeric`
//! heuristic algebraically simplified (see this plan's Global Constraints — same
//! observable result, no per-iteration recomputation of a loop-invariant condition).
use pcbparts_parsers::alternatives::{spec_parsers, SpecParser};
use pcbparts_parsers::subcategory_aliases::{find_similar_subcategories, resolve_subcategory_name};
use pcbparts_search::result::SubcategoryInfo;
use pcbparts_search::spec_filter::attribute_aliases;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};

const VALUE_LIMIT: usize = 50;

pub fn list_attributes(
    conn: &Connection,
    subcategories: &BTreeMap<i64, SubcategoryInfo>,
    subcategory_name_to_id: &HashMap<String, i64>,
    subcategory_id: Option<i64>,
    subcategory_name: Option<&str>,
    sample_size: i64,
) -> Value {
    let resolved_id = match (subcategory_id, subcategory_name) {
        (Some(id), _) => Some(id),
        (None, Some(name)) => {
            let resolved = resolve_subcategory_name(name, subcategory_name_to_id, None);
            if resolved.is_none() {
                let info: HashMap<i64, (String, String)> = subcategories
                    .iter()
                    .map(|(id, i)| (*id, (i.name.clone(), i.category_name.clone().unwrap_or_default())))
                    .collect();
                let similar = find_similar_subcategories(name, subcategory_name_to_id, &info, 5);
                return json!({
                    "error": format!("Subcategory not found: '{name}'"),
                    "hint": "Use search_help() to browse categories and subcategories",
                    "similar_subcategories": similar.iter().map(|s| json!({"id": s.id, "name": s.name, "category": s.category})).collect::<Vec<_>>(),
                });
            }
            resolved
        }
        (None, None) => None,
    };

    let Some(resolved_id) = resolved_id else {
        return json!({
            "error": "Must provide subcategory_id or subcategory_name",
            "hint": "Use search_help() to browse categories and subcategories",
        });
    };

    let Some(subcat_info) = subcategories.get(&resolved_id) else {
        return json!({
            "error": format!("Subcategory ID {resolved_id} not found"),
            "hint": "Use search_help(category=...) to see valid subcategory IDs",
        });
    };

    let mut stmt = conn
        .prepare("SELECT attributes FROM components WHERE subcategory_id = ? LIMIT ?")
        .unwrap();
    let rows = stmt.query_map(rusqlite::params![resolved_id, sample_size], |row| row.get::<_, Option<String>>(0)).unwrap();

    let mut attr_counts: HashMap<String, i64> = HashMap::new();
    let mut attr_values: HashMap<String, Vec<String>> = HashMap::new();
    let mut attr_values_seen: HashMap<String, HashSet<String>> = HashMap::new();
    let mut attr_order: Vec<String> = Vec::new();

    for row in rows.filter_map(|r| r.ok()) {
        let Some(attrs_json) = row.filter(|s| !s.is_empty()) else { continue };
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&attrs_json) else { continue };
        let Some(arr) = parsed.as_array() else { continue };

        for item in arr {
            let Some(pair_arr) = item.as_array() else { continue };
            if pair_arr.len() != 2 {
                continue;
            }
            let Some(name) = pair_arr[0].as_str() else { continue };
            let Some(value) = pair_arr[1].as_str() else { continue };

            if !attr_counts.contains_key(name) {
                attr_order.push(name.to_string());
            }
            *attr_counts.entry(name.to_string()).or_insert(0) += 1;
            let seen = attr_values_seen.entry(name.to_string()).or_default();
            if seen.len() < 100 && seen.insert(value.to_string()) {
                attr_values.entry(name.to_string()).or_default().push(value.to_string());
            }
        }
    }

    // Bind once — spec_parsers() rebuilds a ~300-entry HashMap from scratch on every
    // call, and the loop below would otherwise call it several times per attribute.
    let parsers = spec_parsers();

    let aliases = attribute_aliases();
    let mut alias_lookup: HashMap<String, String> = HashMap::new();
    for (alias, full_names) in &aliases {
        for full_name in full_names {
            // First-wins is safe only because no full attribute name currently appears
            // under two different alias keys in ATTRIBUTE_ALIASES (verified separately).
            // If that data ever changes, iteration order over this HashMap would make
            // the winner non-deterministic and this would need a deterministic tie-break.
            alias_lookup.entry(full_name.to_string()).or_insert_with(|| alias.to_string());
        }
    }

    let mut sorted_names: Vec<(&String, &i64)> = attr_order
        .iter()
        .filter_map(|name| attr_counts.get(name).map(|count| (name, count)))
        .collect();
    sorted_names.sort_by_key(|(_, count)| -**count);

    let mut attributes = Vec::new();
    for (name, count) in sorted_names {
        // Defaults to the attribute's own `name` (not Python's literal `""` default)
        // when not found in alias_lookup. Behaviorally inert: the only downstream use
        // is `aliases.get(alias_target)`, and if `name` isn't a key in alias_lookup it
        // also can't be a valid key into `aliases` by construction — so the default is
        // never actually used as a real lookup key in either language.
        let alias_target = alias_lookup.get(name).map(|s| s.as_str()).unwrap_or(name.as_str());
        let target_full_names: Vec<&str> = aliases.get(alias_target).cloned().unwrap_or_else(|| vec![name.as_str()]);
        let condition = target_full_names.iter().any(|fname| parsers.contains_key(fname));
        let is_numeric_by_alias = condition && aliases.values().any(|full_names| full_names.contains(&name.as_str()));
        let mut is_numeric = parsers.contains_key(name.as_str()) || is_numeric_by_alias;

        // Same divergence as categories.rs::find_by_subcategory: Python's plain
        // truthiness check on SPEC_PARSERS.get(name) treats the sentinel string
        // "special" as truthy and crashes calling it as a function. Matching
        // explicitly on SpecParser::Parser here excludes that case — intentional.
        let values = attr_values.get(name).cloned().unwrap_or_default();
        let parser = match parsers.get(name.as_str()) {
            Some(SpecParser::Parser(f)) => Some(*f),
            _ => alias_lookup.get(name).and_then(|alias| {
                aliases.get(alias.as_str()).and_then(|targets| {
                    targets.iter().find_map(|t| match parsers.get(*t) {
                        Some(SpecParser::Parser(f)) => Some(*f),
                        _ => None,
                    })
                })
            }),
        };

        let mut parsed_values: Vec<(String, Option<f64>)> = if let Some(p) = parser {
            values.iter().map(|v| (v.clone(), p(v))).collect()
        } else {
            values.iter().map(|v| (v.clone(), None)).collect()
        };

        if parser.is_some() && !parsed_values.is_empty() {
            let sampled: Vec<&(String, Option<f64>)> = parsed_values.iter().take(10).collect();
            let numeric_count = sampled.iter().filter(|(_, pv)| pv.is_some()).count();
            is_numeric = numeric_count as f64 >= sampled.len() as f64 * 0.5;
        }

        parsed_values.sort_by(|a, b| match (a.1, b.1) {
            (None, None) => a.0.cmp(&b.0),
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
        });
        let sorted_values: Vec<String> = parsed_values.iter().map(|(v, _)| v.clone()).collect();

        let mut attr_info = json!({
            "name": name,
            "alias": alias_lookup.get(name),
            "type": if is_numeric { "numeric" } else { "string" },
            "count": count,
        });

        if is_numeric {
            attr_info["example_values"] = json!(sorted_values.iter().take(5).collect::<Vec<_>>());
            attr_info["values"] = json!(sorted_values.iter().take(VALUE_LIMIT).collect::<Vec<_>>());
            let parsed_floats: Vec<f64> = parsed_values.iter().filter_map(|(_, pv)| *pv).collect();
            if !parsed_floats.is_empty() {
                attr_info["min"] = json!(parsed_floats.iter().cloned().fold(f64::INFINITY, f64::min));
                attr_info["max"] = json!(parsed_floats.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
            }
        } else {
            attr_info["values"] = json!(sorted_values.iter().take(VALUE_LIMIT).collect::<Vec<_>>());
        }

        if values.len() > VALUE_LIMIT {
            attr_info["total_unique"] = json!(values.len());
        }

        attributes.push(attr_info);
    }

    json!({
        "subcategory_id": resolved_id,
        "subcategory_name": subcat_info.name,
        "category_name": subcat_info.category_name,
        "sample_size": sample_size,
        "attributes": attributes,
    })
}
