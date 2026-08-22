use rusqlite::{Connection, ToSql};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

use super::search::{escape_like, source_url};

const PASSIVE_PREFIXES: &[&str] = &["R", "C", "L", "RN", "FB"];

fn junk_map(v: &str) -> Option<&'static str> {
    match v {
        "R" => Some("resistor"),
        "C" => Some("capacitor"),
        "L" => Some("inductor"),
        "D" => Some("diode"),
        "F" => Some("fuse"),
        "FB" => Some("ferrite bead"),
        _ => None,
    }
}

/// (matched_hood, match_type, partial_matches)
fn match_focus(neighborhoods: &[Value], focus: &str) -> (Option<Value>, Option<&'static str>, Vec<Value>) {
    let focus_lower = focus.to_lowercase();

    for hood in neighborhoods {
        if hood["ref"].as_str().unwrap_or("").to_lowercase() == focus_lower {
            return (Some(hood.clone()), Some("ref"), vec![]);
        }
    }
    for hood in neighborhoods {
        if hood["value"].as_str().unwrap_or("").to_lowercase() == focus_lower {
            return (Some(hood.clone()), Some("exact"), vec![]);
        }
    }
    let mut partial_matches: Vec<Value> = Vec::new();
    for hood in neighborhoods {
        let val_lower = hood["value"].as_str().unwrap_or("").to_lowercase();
        if val_lower.contains(&focus_lower) || focus_lower.contains(&val_lower) {
            partial_matches.push(hood.clone());
        }
    }
    if let Some(first) = partial_matches.first().cloned() {
        return (Some(first), Some("partial"), partial_matches);
    }
    (None, None, vec![])
}

fn clean_junk_values(pins: &Value) -> Value {
    let mut cleaned = serde_json::Map::new();
    if let Some(obj) = pins.as_object() {
        for (pin_name, components) in obj {
            let arr = components.as_array().cloned().unwrap_or_default();
            let cleaned_arr: Vec<Value> = arr
                .into_iter()
                .map(|c| {
                    if let Some(v) = c.get("value").and_then(|v| v.as_str()) {
                        if let Some(mapped) = junk_map(v) {
                            let mut c2 = c.clone();
                            c2["value"] = json!(mapped);
                            return c2;
                        }
                    }
                    c
                })
                .collect();
            cleaned.insert(pin_name.clone(), json!(cleaned_arr));
        }
    }
    Value::Object(cleaned)
}

fn filter_components(conn: &Connection, board_id: i64, include_bom: bool) -> (Vec<Value>, i64) {
    let mut stmt = conn
        .prepare(
            "SELECT ref, value, footprint, description, voltage, tolerance, dielectric, \
             decouples, pullup, pulldown FROM board_components WHERE board_id = ? ORDER BY ref",
        )
        .unwrap();
    let cols = ["ref", "value", "footprint", "description", "voltage", "tolerance", "dielectric", "decouples", "pullup", "pulldown"];
    let all_components: Vec<Value> = stmt
        .query_map([board_id], |row| {
            let mut map = serde_json::Map::new();
            for (i, col) in cols.iter().enumerate() {
                let v: Option<String> = row.get(i)?;
                if let Some(v) = v {
                    map.insert(col.to_string(), json!(v));
                }
            }
            Ok(Value::Object(map))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    if include_bom {
        return (all_components, 0);
    }

    let mut filtered = Vec::new();
    for c in &all_components {
        let ref_str = c["ref"].as_str().unwrap_or("");
        let prefix: String = ref_str.chars().take_while(|ch| ch.is_ascii_alphabetic()).collect();
        let is_passive = PASSIVE_PREFIXES.contains(&prefix.as_str());
        let has_annotation = c.get("decouples").is_some() || c.get("pullup").is_some() || c.get("pulldown").is_some();
        if !is_passive || has_annotation {
            filtered.push(c.clone());
        }
    }
    let omitted = all_components.len() as i64 - filtered.len() as i64;
    (filtered, omitted)
}

fn enrich_neighborhoods(conn: &Connection, board_id: i64, neighborhoods: &[Value]) -> Vec<Value> {
    let ic_refs: Vec<String> = neighborhoods.iter().map(|h| h["ref"].as_str().unwrap_or("").to_string()).collect();
    let mut ic_descriptions: HashMap<String, String> = HashMap::new();
    if !ic_refs.is_empty() {
        let placeholders = vec!["?"; ic_refs.len()].join(",");
        let sql = format!("SELECT ref, description FROM board_components WHERE board_id = ? AND ref IN ({placeholders})");
        let mut stmt = conn.prepare(&sql).unwrap();
        let mut all_params: Vec<&dyn ToSql> = vec![&board_id];
        all_params.extend(ic_refs.iter().map(|r| r as &dyn ToSql));
        let mut rows = stmt.query(all_params.as_slice()).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let r: String = row.get(0).unwrap();
            let d: Option<String> = row.get(1).unwrap();
            if let Some(d) = d {
                ic_descriptions.insert(r, d);
            }
        }
    }

    neighborhoods
        .iter()
        .map(|h| {
            let ref_str = h["ref"].as_str().unwrap_or("").to_string();
            let pin_count = h["pins"].as_object().map(|o| o.len()).unwrap_or(0);
            let mut entry = json!({
                "ref": ref_str,
                "value": h["value"],
                "pin_count": pin_count,
            });
            if let Some(desc) = ic_descriptions.get(&ref_str) {
                entry["description"] = json!(desc);
            }
            entry
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn get_board(
    conn: &Connection,
    slug: &str,
    include_raw: bool,
    include_bom: bool,
    focus: Option<&str>,
) -> Option<Value> {
    if slug.is_empty() {
        return None;
    }

    let mut cols = "id, slug, name, org, org_display, source, format, description, key_coverage, \
                    layers, width_mm, height_mm, min_trace, min_clearance, min_drill, min_via, \
                    component_count, ic_count, net_count, neighborhoods_json"
        .to_string();
    if include_raw {
        cols.push_str(", nets_json, positions_json, copper_pours_json");
    }

    let sql = format!("SELECT {cols} FROM boards WHERE slug = ?");
    let row_result: rusqlite::Result<(
        i64, String, String, Option<String>, Option<String>, Option<String>, Option<String>,
        Option<String>, Option<String>, Option<i64>, Option<f64>, Option<f64>, Option<String>,
        Option<String>, Option<String>, Option<String>, i64, i64, i64, Option<String>,
    )> = conn.query_row(&sql, [slug], |row| {
        Ok((
            row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?,
            row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?,
            row.get(12)?, row.get(13)?, row.get(14)?, row.get(15)?, row.get(16)?, row.get(17)?,
            row.get(18)?, row.get(19)?,
        ))
    });
    let (
        board_id, slug_v, name, org, org_display, source, format_, description, key_coverage,
        layers, width_mm, height_mm, min_trace, min_clearance, min_drill, min_via,
        component_count, ic_count, net_count, neighborhoods_json,
    ) = match row_result {
        Ok(r) => r,
        Err(_) => return None,
    };

    let tags: Vec<String> = {
        let mut stmt = conn.prepare("SELECT tag FROM board_tags WHERE board_id = ? ORDER BY tag").unwrap();
        stmt.query_map([board_id], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
    };
    let key_ics: Vec<String> = {
        let mut stmt = conn.prepare("SELECT ic FROM board_key_ics WHERE board_id = ? ORDER BY ic").unwrap();
        stmt.query_map([board_id], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
    };

    let neighborhoods: Vec<Value> = neighborhoods_json
        .as_deref()
        .map(|s| serde_json::from_str(s).unwrap_or_default())
        .unwrap_or_default();

    let mut result = json!({
        "slug": slug_v,
        "name": name,
        "org": org,
        "org_display": org_display,
        "source": source,
        "source_url": source_url(source.as_deref()),
        "format": format_,
        "description": description,
        "key_coverage": key_coverage,
        "layers": layers,
        "width_mm": width_mm,
        "height_mm": height_mm,
        "min_trace": min_trace,
        "min_clearance": min_clearance,
        "min_drill": min_drill,
        "min_via": min_via,
        "component_count": component_count,
        "ic_count": ic_count,
        "net_count": net_count,
        "tags": tags,
        "key_ics": key_ics,
    });

    if let Some(focus_term) = focus {
        let (matched, match_type, partial_matches) = match_focus(&neighborhoods, focus_term);

        if let Some(mut matched) = matched {
            matched["pins"] = clean_junk_values(&matched["pins"]);
            result["focus"] = matched.clone();
            result["focus_match_type"] = json!(match_type);

            let matched_value = matched["value"].as_str().unwrap_or("").to_string();
            let mut consensus = get_consensus(conn, &matched_value);
            if consensus.is_none() && match_type == Some("partial") && focus_term != matched_value {
                consensus = get_consensus(conn, focus_term);
            }
            if let Some(c) = consensus {
                result["consensus"] = c;
            }

            if match_type == Some("partial") && partial_matches.len() > 1 {
                let mut seen_values: HashSet<String> = HashSet::new();
                seen_values.insert(matched_value);
                let mut alternatives: Vec<Value> = Vec::new();
                for alt in &partial_matches[1..] {
                    let alt_value = alt["value"].as_str().unwrap_or("").to_string();
                    if seen_values.insert(alt_value.clone()) {
                        alternatives.push(json!({"ref": alt["ref"], "value": alt_value}));
                    }
                }
                if !alternatives.is_empty() {
                    result["focus_alternatives"] = json!(alternatives);
                }
            }
        } else {
            let available: Vec<Value> = if !neighborhoods.is_empty() {
                result["focus_error"] = json!(format!(
                    "IC '{}' not found on this board",
                    &focus_term.chars().take(50).collect::<String>()
                ));
                neighborhoods
                    .iter()
                    .map(|h| json!({"ref": h["ref"], "value": h["value"]}))
                    .collect()
            } else {
                result["focus_error"] = json!(format!(
                    "IC '{}' not found — this board has no parsed IC neighborhoods. Try include_bom=True to see all components.",
                    &focus_term.chars().take(50).collect::<String>()
                ));
                key_ics.iter().map(|ic| json!({"value": ic})).collect()
            };
            result["available_ics"] = json!(available);
        }
    } else {
        let (components, passives_omitted) = filter_components(conn, board_id, include_bom);
        result["components"] = json!(components);
        if passives_omitted > 0 {
            result["passives_omitted"] = json!(passives_omitted);
        }
        result["neighborhoods"] = json!(enrich_neighborhoods(conn, board_id, &neighborhoods));
    }

    if include_raw {
        // Columns appended after neighborhoods_json in the SELECT above; re-query directly
        // since the tuple decode above only captured the base 20 columns.
        let (nets, positions, copper_pours): (Option<String>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT nets_json, positions_json, copper_pours_json FROM boards WHERE id = ?",
                [board_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        result["nets"] = nets.map(|s| serde_json::from_str::<Value>(&s).unwrap_or(json!([]))).unwrap_or(json!([]));
        result["positions"] = positions.map(|s| serde_json::from_str::<Value>(&s).unwrap_or(json!([]))).unwrap_or(json!([]));
        result["copper_pours"] = copper_pours.map(|s| serde_json::from_str::<Value>(&s).unwrap_or(json!([]))).unwrap_or(json!([]));
    }

    Some(result)
}

pub fn get_consensus(conn: &Connection, ic_name: &str) -> Option<Value> {
    let escaped = escape_like(ic_name);
    let ic_pattern = format!("%{escaped}%");
    let candidate_ids: Vec<i64> = {
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT board_id FROM (
                    SELECT board_id FROM board_key_ics WHERE ic LIKE ? ESCAPE '\\'
                    UNION
                    SELECT board_id FROM board_components WHERE ref LIKE 'U%' AND value LIKE ? ESCAPE '\\'
                )",
            )
            .unwrap();
        stmt.query_map(rusqlite::params![ic_pattern, ic_pattern], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };

    if candidate_ids.is_empty() {
        return None;
    }

    let placeholders = vec!["?"; candidate_ids.len()].join(",");
    let sql = format!(
        "SELECT slug, neighborhoods_json FROM boards WHERE id IN ({placeholders}) AND neighborhoods_json IS NOT NULL"
    );
    let id_refs: Vec<&dyn ToSql> = candidate_ids.iter().map(|i| i as &dyn ToSql).collect();
    let rows: Vec<(String, String)> = {
        let mut stmt = conn.prepare(&sql).unwrap();
        stmt.query_map(id_refs.as_slice(), |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };

    let ic_lower = ic_name.to_lowercase();
    let mut all_hoods: Vec<Value> = Vec::new();
    let mut board_slugs: Vec<String> = Vec::new();

    for (slug, hoods_json) in rows {
        let hoods: Vec<Value> = serde_json::from_str(&hoods_json).unwrap_or_default();
        for h in hoods {
            if h["value"].as_str().unwrap_or("").to_lowercase().contains(&ic_lower) {
                all_hoods.push(h);
                board_slugs.push(slug.clone());
                break;
            }
        }
    }

    if all_hoods.len() < 2 {
        return None;
    }

    let total = all_hoods.len();
    let mut pin_consensus: HashMap<String, (i64, HashMap<String, i64>)> = HashMap::new();
    let mut decap_boards: HashMap<String, HashSet<usize>> = HashMap::new();

    for (i, hood) in all_hoods.iter().enumerate() {
        if let Some(pins) = hood["pins"].as_object() {
            for (pin_name, components) in pins {
                if pin_name == "_decoupling" {
                    if let Some(arr) = components.as_array() {
                        for c in arr {
                            let value = c["value"].as_str().unwrap_or("").to_string();
                            decap_boards.entry(value).or_default().insert(i);
                        }
                    }
                    continue;
                }
                let entry = pin_consensus.entry(pin_name.clone()).or_insert((0, HashMap::new()));
                entry.0 += 1;
                if let Some(arr) = components.as_array() {
                    for c in arr {
                        let value = c["value"].as_str().unwrap_or("");
                        let role = c["role"].as_str().unwrap_or("");
                        let key = format!("{value} [{role}]");
                        *entry.1.entry(key).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    const MAX_CONSENSUS_PINS: usize = 30;
    let mut eligible_pins: Vec<(&String, &(i64, HashMap<String, i64>))> =
        pin_consensus.iter().filter(|(_, (count, _))| *count >= 2).collect();
    eligible_pins.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    let pins_truncated = eligible_pins.len() > MAX_CONSENSUS_PINS;
    eligible_pins.truncate(MAX_CONSENSUS_PINS);

    let mut pins_result = serde_json::Map::new();
    for (pin_name, (count, components)) in eligible_pins {
        let mut sorted: Vec<(&String, &i64)> = components.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        let top_choices: Vec<Value> = sorted
            .into_iter()
            .take(5)
            .map(|(vr, cnt)| {
                json!({"value_role": vr, "count": cnt, "pct": ((*cnt as f64) * 100.0 / total as f64).round() as i64})
            })
            .collect();
        pins_result.insert(
            pin_name.clone(),
            json!({"boards_with_pin": count, "top_choices": top_choices}),
        );
    }

    let mut decoupling: Vec<(String, usize)> =
        decap_boards.into_iter().map(|(val, boards)| (val, boards.len())).collect();
    decoupling.sort_by(|a, b| b.1.cmp(&a.1));
    decoupling.truncate(5);
    let decoupling: Vec<Value> = decoupling
        .into_iter()
        .map(|(val, boards)| {
            json!({"value": val, "boards": boards, "pct": ((boards as f64) * 100.0 / total as f64).round() as i64})
        })
        .collect();

    let mut result = json!({
        "ic": ic_name,
        "board_count": total,
        "boards": board_slugs,
        "decoupling": decoupling,
        "pins": pins_result,
    });
    if pins_truncated {
        result["pins_truncated"] = json!(true);
    }
    Some(result)
}

pub fn get_tag_consensus(conn: &Connection, tag: &str) -> Option<Value> {
    let rows: Vec<(String, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT bk.ic, GROUP_CONCAT(DISTINCT b.slug) as board_slugs
                 FROM board_tags bt
                 JOIN board_key_ics bk ON bt.board_id = bk.board_id
                 JOIN boards b ON bt.board_id = b.id
                 WHERE bt.tag = ?
                 GROUP BY bk.ic
                 ORDER BY COUNT(DISTINCT bk.board_id) DESC",
            )
            .unwrap();
        stmt.query_map([tag], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };

    if rows.is_empty() {
        return None;
    }

    let board_count: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT board_id) FROM board_tags WHERE tag = ?",
            [tag],
            |r| r.get(0),
        )
        .unwrap_or(0);

    if board_count < 2 {
        return None;
    }

    let mut top_ics: Vec<Value> = Vec::new();
    for (ic, slugs_str) in rows.into_iter().take(10) {
        let mut boards_list: Vec<String> = slugs_str.split(',').map(|s| s.to_string()).collect::<HashSet<_>>().into_iter().collect();
        boards_list.sort();
        let ic_boards = boards_list.len() as i64;
        top_ics.push(json!({
            "ic": ic,
            "boards": ic_boards,
            "pct": ((ic_boards as f64) * 100.0 / board_count as f64).round() as i64,
            "example_boards": boards_list.into_iter().take(3).collect::<Vec<_>>(),
        }));
    }

    Some(json!({
        "tag": tag,
        "board_count": board_count,
        "top_ics": top_ics,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boards::fixtures::test_db;

    // --- get_board: default mode ---
    #[test]
    fn test_get_board_basic() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", false, false, None).unwrap();
        assert_eq!(b["name"], "Test ESP32 Board");
        assert_eq!(b["slug"], "test-esp32-board");
    }
    #[test]
    fn test_components_filtered_by_default() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", false, false, None).unwrap();
        let refs: Vec<&str> = b["components"].as_array().unwrap().iter().map(|c| c["ref"].as_str().unwrap()).collect();
        assert!(refs.contains(&"U1"));
        assert!(refs.contains(&"U2"));
        assert!(refs.contains(&"C1"));
        assert!(refs.contains(&"C2"));
        assert!(refs.contains(&"R3"));
        assert!(!refs.contains(&"R1"));
        assert!(!refs.contains(&"R2"));
        assert!(!refs.contains(&"C3"));
        assert_eq!(b["passives_omitted"], 3);
    }
    #[test]
    fn test_include_bom_returns_all() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", false, true, None).unwrap();
        assert_eq!(b["components"].as_array().unwrap().len(), 8);
        assert!(b.get("passives_omitted").is_none());
    }
    #[test]
    fn test_neighborhoods_summary() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", false, false, None).unwrap();
        for hood in b["neighborhoods"].as_array().unwrap() {
            assert!(hood.get("ref").is_some());
            assert!(hood.get("value").is_some());
            assert!(hood.get("pin_count").is_some());
        }
    }
    #[test]
    fn test_no_nets_in_default() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", false, false, None).unwrap();
        assert!(b.get("nets").is_none());
        assert!(b.get("positions").is_none());
        assert!(b.get("copper_pours").is_none());
    }
    #[test]
    fn test_tags_and_key_ics() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", false, false, None).unwrap();
        assert!(b["tags"].as_array().unwrap().iter().any(|v| v == "sensors"));
        assert!(b["key_ics"].as_array().unwrap().iter().any(|v| v == "ESP32-S3"));
    }
    #[test]
    fn test_nonexistent_returns_none() {
        let conn = test_db();
        assert!(get_board(&conn, "nonexistent-slug-12345", false, false, None).is_none());
    }
    #[test]
    fn test_empty_slug_returns_none() {
        let conn = test_db();
        assert!(get_board(&conn, "", false, false, None).is_none());
    }

    // --- get_board: focus mode ---
    #[test]
    fn test_focus_by_ic_name() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", false, false, Some("ESP32-S3")).unwrap();
        assert_eq!(b["focus"]["value"], "ESP32-S3");
        assert!(b.get("components").is_none());
    }
    #[test]
    fn test_focus_by_ref() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", false, false, Some("U1")).unwrap();
        assert_eq!(b["focus"]["ref"], "U1");
    }
    #[test]
    fn test_focus_partial_match() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", false, false, Some("ESP32")).unwrap();
        assert!(b.get("focus").is_some());
    }
    #[test]
    fn test_focus_nonexistent_ic() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", false, false, Some("NONEXISTENT_IC")).unwrap();
        assert!(b.get("focus_error").is_some());
        assert!(b.get("available_ics").is_some());
    }
    #[test]
    fn test_focus_auto_consensus() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", false, false, Some("MCP73831")).unwrap();
        assert!(b.get("focus").is_some());
        assert!(b.get("consensus").is_some());
        assert_eq!(b["consensus"]["ic"], "MCP73831");
        assert_eq!(b["consensus"]["board_count"], 2);
    }
    #[test]
    fn test_focus_no_consensus_for_single_board_ic() {
        let conn = test_db();
        let b = get_board(&conn, "adafruit-motor-shield", false, false, Some("DRV8825")).unwrap();
        assert!(b.get("focus").is_some());
        assert!(b.get("consensus").is_none());
    }
    #[test]
    fn test_match_type_ref() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", false, false, Some("U1")).unwrap();
        assert_eq!(b["focus_match_type"], "ref");
    }
    #[test]
    fn test_match_type_exact() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", false, false, Some("ESP32-S3")).unwrap();
        assert_eq!(b["focus_match_type"], "exact");
    }
    #[test]
    fn test_match_type_partial() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", false, false, Some("ESP32")).unwrap();
        assert_eq!(b["focus_match_type"], "partial");
    }
    #[test]
    fn test_match_type_not_present_on_miss() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", false, false, Some("NONEXISTENT_IC")).unwrap();
        assert!(b.get("focus_match_type").is_none());
    }
    #[test]
    fn test_no_alternatives_for_unique_partial() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", false, false, Some("ESP32")).unwrap();
        assert!(b.get("focus").is_some());
        assert!(b.get("focus_alternatives").is_none());
    }
    #[test]
    fn test_no_alternatives_for_exact_match() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", false, false, Some("MCP73831")).unwrap();
        assert_eq!(b["focus_match_type"], "exact");
        assert!(b.get("focus_alternatives").is_none());
    }
    #[test]
    fn test_focus_on_no_neighborhood_board() {
        let conn = test_db();
        let b = get_board(&conn, "minimal-led-driver", false, false, Some("TPS61169")).unwrap();
        assert!(b["focus_error"].as_str().unwrap().contains("no parsed IC neighborhoods"));
        let avail_values: Vec<&str> = b["available_ics"].as_array().unwrap().iter().map(|a| a["value"].as_str().unwrap()).collect();
        assert!(avail_values.contains(&"TPS61169"));
    }

    // --- get_board: raw mode ---
    #[test]
    fn test_raw_mode() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", true, false, None).unwrap();
        assert_eq!(b["nets"].as_array().unwrap().len(), 5);
        assert_eq!(b["positions"].as_array().unwrap().len(), 2);
        assert_eq!(b["copper_pours"].as_array().unwrap().len(), 1);
    }
    #[test]
    fn test_raw_plus_focus() {
        let conn = test_db();
        let b = get_board(&conn, "test-esp32-board", true, false, Some("ESP32-S3")).unwrap();
        assert!(b.get("focus").is_some());
        assert!(b.get("nets").is_some());
    }

    // --- get_consensus ---
    #[test]
    fn test_consensus_found() {
        let conn = test_db();
        let c = get_consensus(&conn, "MCP73831").unwrap();
        assert_eq!(c["ic"], "MCP73831");
        assert_eq!(c["board_count"], 2);
    }
    #[test]
    fn test_consensus_boards() {
        let conn = test_db();
        let c = get_consensus(&conn, "MCP73831").unwrap();
        let boards: HashSet<String> = c["boards"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        assert_eq!(boards, HashSet::from(["test-esp32-board".to_string(), "sparkfun-mcp73831-charger".to_string()]));
    }
    #[test]
    fn test_consensus_nonexistent() {
        let conn = test_db();
        assert!(get_consensus(&conn, "NONEXISTENT_IC_99999").is_none());
    }
    #[test]
    fn test_consensus_single_board_returns_none() {
        let conn = test_db();
        assert!(get_consensus(&conn, "DRV8825").is_none());
    }

    // --- get_tag_consensus ---
    #[test]
    fn test_battery_charging_consensus() {
        let conn = test_db();
        let c = get_tag_consensus(&conn, "battery-charging").unwrap();
        assert_eq!(c["tag"], "battery-charging");
        assert_eq!(c["board_count"], 2);
        let ics: Vec<&str> = c["top_ics"].as_array().unwrap().iter().map(|e| e["ic"].as_str().unwrap()).collect();
        assert!(ics.contains(&"MCP73831"));
    }
    #[test]
    fn test_top_ics_shape() {
        let conn = test_db();
        let c = get_tag_consensus(&conn, "battery-charging").unwrap();
        for entry in c["top_ics"].as_array().unwrap() {
            assert!(entry.get("ic").is_some());
            assert!(entry.get("boards").is_some());
            assert!(entry.get("pct").is_some());
            assert!(entry.get("example_boards").is_some());
        }
    }
    #[test]
    fn test_nonexistent_tag_consensus() {
        let conn = test_db();
        assert!(get_tag_consensus(&conn, "nonexistent-tag-xyz").is_none());
    }
    #[test]
    fn test_motor_control_single_board() {
        let conn = test_db();
        assert!(get_tag_consensus(&conn, "motor-control").is_none());
    }
    #[test]
    fn test_sensors_consensus() {
        let conn = test_db();
        let c = get_tag_consensus(&conn, "sensors").unwrap();
        assert_eq!(c["board_count"], 2);
    }
}
