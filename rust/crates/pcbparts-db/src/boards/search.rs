use rusqlite::{Connection, ToSql};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

pub(crate) fn escape_like(value: &str) -> String {
    value.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

pub(crate) fn source_url(source: Option<&str>) -> Option<String> {
    source.map(|s| format!("https://github.com/{s}"))
}

fn fts_stop_words() -> &'static [&'static str] {
    &[
        "a", "an", "the", "and", "or", "of", "for", "with", "in", "on", "to", "is", "it", "by",
        "at", "from", "as", "be", "my", "me", "do", "no", "so", "up", "if", "am", "are", "was",
        "has", "have", "had", "not", "but", "will", "would", "could", "should", "what", "which",
        "that", "this", "these", "those", "how", "when", "where", "who", "than", "then", "also",
        "just", "very", "really", "any", "some", "about", "like", "into", "over", "such",
        "board", "boards", "design", "circuit", "schematic", "pcb", "reference", "find", "show",
        "get", "list", "using", "project", "open", "source", "hardware", "voltage", "regulator",
        "converter", "output", "input", "signal", "interface", "module", "chip", "defined",
        "panel", "software",
    ]
}

fn term_aliases() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("eink", "e-ink"), ("epaper", "e-paper"), ("ebike", "e-bike"),
        ("esp32s3", "ESP32-S3"), ("esp32c3", "ESP32-C3"), ("esp32c6", "ESP32-C6"),
        ("esp32s2", "ESP32-S2"), ("nrf52", "nRF52"), ("rs485", "RS-485"), ("rs232", "RS-232"),
        ("usbc", "USB-C"), ("bluetooth", "BLE"), ("ble", "BLE"), ("opamp", "op-amp"),
        ("mosfet", "FET"), ("synthesizer", "synth"), ("oscilloscope", "scope"),
        ("brushless", "BLDC"), ("accelerometer", "IMU"), ("gyroscope", "IMU"),
        ("modbus", "RS-485"), ("amplifier", "amp"), ("thermocouple", "thermocouple"),
        ("lipo", "battery"), ("neopixel", "WS2812"), ("addressable", "WS2812"),
        ("quadcopter", "drone"), ("servo", "motor"), ("stepper", "stepper"),
        ("h-bridge", "H-bridge"), ("hbridge", "H-bridge"), ("lidar", "LIDAR"), ("rtc", "RTC"),
        ("zigbee", "Zigbee"), ("ethernet", "ethernet"), ("oled", "OLED"), ("tft", "TFT"),
        ("sdcard", "SD-card"), ("microsd", "SD-card"),
    ])
}

fn synonym_groups() -> HashMap<&'static str, Vec<&'static str>> {
    HashMap::from([
        ("e-ink", vec!["ink", "paper", "EPD", "eink"]),
        ("e-paper", vec!["ink", "paper", "EPD", "eink"]),
        ("epd", vec!["ink", "paper", "EPD", "eink"]),
        ("eink", vec!["ink", "paper", "EPD", "eink"]),
        ("epaper", vec!["ink", "paper", "EPD", "eink"]),
    ])
}

pub fn sanitize_fts_query(query: &str) -> String {
    let clean = query.replace('"', "").replace('\'', "");
    let stop_words = fts_stop_words();
    let aliases = term_aliases();
    let syn_groups = synonym_groups();

    let mut parts_list: Vec<String> = Vec::new();
    for t in clean.split_whitespace() {
        let t_lower = t.to_lowercase();
        if t.chars().count() < 2 || stop_words.contains(&t_lower.as_str()) {
            continue;
        }

        let joined = t_lower.replace('-', "");
        if let Some(group) = syn_groups.get(t_lower.as_str()).or_else(|| syn_groups.get(joined.as_str())) {
            let or_parts: Vec<String> = group.iter().map(|s| format!("\"{s}\"*")).collect();
            parts_list.push(format!("({})", or_parts.join(" OR ")));
            continue;
        }

        let mut term = t.to_string();
        if let Some(alias) = aliases.get(t_lower.as_str()) {
            term = alias.to_string();
        }

        if term.contains('-') {
            for p in term.split('-') {
                if p.chars().count() >= 2 && !stop_words.contains(&p.to_lowercase().as_str()) {
                    if p.chars().count() <= 3 {
                        parts_list.push(format!("\"{p}\""));
                    } else {
                        parts_list.push(format!("\"{p}\"*"));
                    }
                }
            }
        } else if term.chars().count() <= 3 {
            parts_list.push(format!("\"{term}\""));
        } else {
            parts_list.push(format!("\"{term}\"*"));
        }
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut unique: Vec<String> = Vec::new();
    for p in parts_list {
        if seen.insert(p.clone()) {
            unique.push(p);
        }
    }
    unique.join(" AND ")
}

pub fn get_stats(conn: &Connection) -> Value {
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM boards", [], |r| r.get(0)).unwrap();

    let mut formats = serde_json::Map::new();
    {
        let mut stmt = conn.prepare("SELECT format, COUNT(*) FROM boards GROUP BY format ORDER BY 2 DESC").unwrap();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let f: String = row.get(0).unwrap();
            let c: i64 = row.get(1).unwrap();
            formats.insert(f, json!(c));
        }
    }

    let mut top_orgs = serde_json::Map::new();
    {
        let mut stmt = conn.prepare("SELECT org_display, COUNT(*) FROM boards GROUP BY org_display ORDER BY 2 DESC LIMIT 10").unwrap();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let o: String = row.get(0).unwrap();
            let c: i64 = row.get(1).unwrap();
            top_orgs.insert(o, json!(c));
        }
    }

    let mut top_tags = serde_json::Map::new();
    {
        let mut stmt = conn.prepare("SELECT tag, COUNT(*) FROM board_tags GROUP BY tag ORDER BY 2 DESC LIMIT 15").unwrap();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let t: String = row.get(0).unwrap();
            let c: i64 = row.get(1).unwrap();
            top_tags.insert(t, json!(c));
        }
    }

    json!({
        "total_boards": total,
        "formats": formats,
        "top_orgs": top_orgs,
        "top_tags": top_tags,
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BoardSummary {
    pub slug: String,
    pub name: String,
    pub org: Option<String>,
    pub org_display: Option<String>,
    pub source: Option<String>,
    pub source_url: Option<String>,
    pub format: Option<String>,
    pub description: Option<String>,
    pub key_coverage: Option<String>,
    pub tags: Vec<String>,
    pub key_ics: Vec<String>,
    pub layers: Option<i64>,
    pub width_mm: Option<f64>,
    pub height_mm: Option<f64>,
    pub component_count: i64,
    pub ic_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_by: Option<Vec<String>>,
}

pub struct SearchBoardsResult {
    pub total: i64,
    pub results: Vec<BoardSummary>,
}

#[allow(clippy::too_many_arguments)]
pub fn search_boards(
    conn: &Connection,
    query: Option<&str>,
    component: Option<&str>,
    tag: Option<&[&str]>,
    org: Option<&str>,
    layers: Option<i64>,
    limit: i64,
) -> SearchBoardsResult {
    let query = query.filter(|q| !q.trim().is_empty());
    let component = component.filter(|c| !c.trim().is_empty());
    let org = org.filter(|o| !o.trim().is_empty());

    let select_cols = "b.id, b.slug, b.name, b.org, b.org_display, b.source, b.format, \
                       b.description, b.key_coverage, b.layers, b.width_mm, b.height_mm, \
                       b.component_count, b.ic_count";

    let mut where_clauses: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    let mut joins: Vec<String> = Vec::new();

    let mut has_fts = false;
    let mut component_boost = false;

    if let Some(tags) = tag {
        let tags = &tags[..tags.len().min(10)];
        for (i, t) in tags.iter().enumerate() {
            joins.push(format!(
                "JOIN board_tags bt{i} ON bt{i}.board_id = b.id AND bt{i}.tag = ?"
            ));
            params.push(Box::new(t.to_string()));
        }
    }

    if let Some(o) = org {
        where_clauses.push("(b.org = ? COLLATE NOCASE OR b.org_display = ? COLLATE NOCASE)".to_string());
        params.push(Box::new(o.to_string()));
        params.push(Box::new(o.to_string()));
    }

    if let Some(l) = layers {
        where_clauses.push("b.layers = ?".to_string());
        params.push(Box::new(l));
    }

    if let Some(c) = component {
        component_boost = true;
        let comp_pattern = format!("%{}%", escape_like(c));
        where_clauses.push(
            "(b.id IN (SELECT board_id FROM board_key_ics WHERE ic LIKE ? ESCAPE '\\') \
             OR b.id IN (SELECT board_id FROM board_components WHERE value LIKE ? ESCAPE '\\'))"
                .to_string(),
        );
        params.push(Box::new(comp_pattern.clone()));
        params.push(Box::new(comp_pattern));
    }

    let mut fts_query: Option<String> = None;
    if let Some(q) = query {
        let sanitized = sanitize_fts_query(q);
        if !sanitized.is_empty() {
            has_fts = true;
            let like_pattern = format!("%{}%", escape_like(q.trim()));
            where_clauses.push(
                "(b.slug IN (SELECT slug FROM boards_fts WHERE boards_fts MATCH ?) \
                 OR b.key_ics_text LIKE ? ESCAPE '\\' OR b.all_ics_text LIKE ? ESCAPE '\\')"
                    .to_string(),
            );
            params.push(Box::new(sanitized.clone()));
            params.push(Box::new(like_pattern.clone()));
            params.push(Box::new(like_pattern));
            fts_query = Some(sanitized);
        }
    }

    let join_sql = joins.join("\n    ");
    let where_sql = if where_clauses.is_empty() { "1=1".to_string() } else { where_clauses.join(" AND ") };

    let mut order_parts: Vec<String> = Vec::new();
    let mut order_params: Vec<Box<dyn ToSql>> = Vec::new();

    if has_fts {
        if let Some(ref fq) = fts_query {
            order_parts.push(
                "COALESCE((SELECT bm25(boards_fts, 10.0, 10.0, 5.0, 5.0, 8.0, 8.0, 3.0, 4.0) \
                 FROM boards_fts WHERE boards_fts MATCH ? AND boards_fts.slug = b.slug), 0)"
                    .to_string(),
            );
            order_params.push(Box::new(fq.clone()));
        }
    }
    if component_boost {
        if let Some(c) = component {
            let comp_pattern = format!("%{}%", escape_like(c));
            order_parts.push(
                "CASE WHEN b.id IN (SELECT board_id FROM board_key_ics WHERE ic LIKE ? ESCAPE '\\') THEN 0 ELSE 1 END"
                    .to_string(),
            );
            order_params.push(Box::new(comp_pattern));
        }
    }
    order_parts.push("b.component_count DESC".to_string());
    order_parts.push("b.name ASC".to_string());
    let order_by = format!("ORDER BY {}", order_parts.join(", "));

    let data_sql = format!(
        "SELECT DISTINCT {select_cols} FROM boards b {join_sql} WHERE {where_sql} {order_by} LIMIT ?"
    );

    let param_refs: Vec<&dyn ToSql> = params.iter().map(|b| b.as_ref()).collect();
    let order_refs: Vec<&dyn ToSql> = order_params.iter().map(|b| b.as_ref()).collect();
    let mut data_params: Vec<&dyn ToSql> = param_refs.clone();
    data_params.extend(order_refs);
    let limit_box: Box<dyn ToSql> = Box::new(limit);
    data_params.push(limit_box.as_ref());

    let mut stmt = match conn.prepare(&data_sql) {
        Ok(s) => s,
        Err(_) => return SearchBoardsResult { total: 0, results: vec![] },
    };
    type Row = (
        i64, String, String, Option<String>, Option<String>, Option<String>, Option<String>,
        Option<String>, Option<String>, Option<i64>, Option<f64>, Option<f64>, i64, i64,
    );
    let rows: Vec<Row> = match stmt.query_map(data_params.as_slice(), |row| {
        Ok((
            row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?,
            row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?,
            row.get(12)?, row.get(13)?,
        ))
    }) {
        Ok(mapped) => match mapped.collect::<Result<_, _>>() {
            Ok(v) => v,
            Err(_) => return SearchBoardsResult { total: 0, results: vec![] },
        },
        Err(_) => return SearchBoardsResult { total: 0, results: vec![] },
    };

    let total: i64 = if (rows.len() as i64) < limit {
        rows.len() as i64
    } else {
        let count_sql = format!("SELECT COUNT(DISTINCT b.id) FROM boards b {join_sql} WHERE {where_sql}");
        conn.query_row(&count_sql, param_refs.as_slice(), |r| r.get(0)).unwrap_or(0)
    };

    if rows.is_empty() {
        return SearchBoardsResult { total, results: vec![] };
    }

    let board_ids: Vec<i64> = rows.iter().map(|r| r.0).collect();
    let placeholders = vec!["?"; board_ids.len()].join(",");
    let id_refs: Vec<&dyn ToSql> = board_ids.iter().map(|i| i as &dyn ToSql).collect();

    let mut tags_by_board: HashMap<i64, Vec<String>> = HashMap::new();
    {
        let sql = format!("SELECT board_id, tag FROM board_tags WHERE board_id IN ({placeholders}) ORDER BY tag");
        let mut stmt = conn.prepare(&sql).unwrap();
        let mut r = stmt.query(id_refs.as_slice()).unwrap();
        while let Some(row) = r.next().unwrap() {
            tags_by_board.entry(row.get(0).unwrap()).or_default().push(row.get(1).unwrap());
        }
    }

    let mut ics_by_board: HashMap<i64, Vec<String>> = HashMap::new();
    {
        let sql = format!("SELECT board_id, ic FROM board_key_ics WHERE board_id IN ({placeholders}) ORDER BY ic");
        let mut stmt = conn.prepare(&sql).unwrap();
        let mut r = stmt.query(id_refs.as_slice()).unwrap();
        while let Some(row) = r.next().unwrap() {
            ics_by_board.entry(row.get(0).unwrap()).or_default().push(row.get(1).unwrap());
        }
    }

    let mut key_ic_match_ids: HashSet<i64> = HashSet::new();
    let mut comp_val_match_ids: HashSet<i64> = HashSet::new();
    if let Some(c) = component {
        let comp_pattern = format!("%{}%", escape_like(c));
        {
            let sql = format!(
                "SELECT board_id FROM board_key_ics WHERE ic LIKE ? ESCAPE '\\' AND board_id IN ({placeholders})"
            );
            let mut stmt = conn.prepare(&sql).unwrap();
            let mut all_params: Vec<&dyn ToSql> = vec![&comp_pattern];
            all_params.extend(id_refs.iter());
            let mut r = stmt.query(all_params.as_slice()).unwrap();
            while let Some(row) = r.next().unwrap() {
                key_ic_match_ids.insert(row.get(0).unwrap());
            }
        }
        {
            let sql = format!(
                "SELECT DISTINCT board_id FROM board_components WHERE value LIKE ? ESCAPE '\\' AND board_id IN ({placeholders})"
            );
            let mut stmt = conn.prepare(&sql).unwrap();
            let mut all_params: Vec<&dyn ToSql> = vec![&comp_pattern];
            all_params.extend(id_refs.iter());
            let mut r = stmt.query(all_params.as_slice()).unwrap();
            while let Some(row) = r.next().unwrap() {
                comp_val_match_ids.insert(row.get(0).unwrap());
            }
        }
    }

    let mut results = Vec::with_capacity(rows.len());
    for (id, slug, name, org_v, org_display, source, format_, description, key_coverage, layers_v, width_mm, height_mm, component_count, ic_count) in rows {
        let mut hints: Vec<String> = Vec::new();
        if let Some(c) = component {
            if key_ic_match_ids.contains(&id) {
                hints.push(format!("key IC: {c}"));
            } else if comp_val_match_ids.contains(&id) {
                hints.push(format!("component: {c}"));
            }
        }
        if has_fts {
            hints.push("text match".to_string());
        }
        if let Some(tags) = tag {
            hints.push(format!("tag: {}", tags.join(", ")));
        }
        if let Some(o) = org {
            hints.push(format!("org: {o}"));
        }
        if let Some(l) = layers {
            hints.push(format!("layers: {l}"));
        }

        results.push(BoardSummary {
            slug: slug.clone(),
            name,
            org: org_v,
            org_display,
            source: source.clone(),
            source_url: source_url(source.as_deref()),
            format: format_,
            description,
            key_coverage,
            tags: tags_by_board.get(&id).cloned().unwrap_or_default(),
            key_ics: ics_by_board.get(&id).cloned().unwrap_or_default(),
            layers: layers_v,
            width_mm,
            height_mm,
            component_count,
            ic_count,
            matched_by: if hints.is_empty() { None } else { Some(hints) },
        });
    }

    SearchBoardsResult { total, results }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boards::fixtures::test_db;

    // --- sanitize_fts_query ---
    #[test]
    fn test_simple_terms() {
        assert_eq!(sanitize_fts_query("ESP32 WiFi"), "\"ESP32\"* AND \"WiFi\"*");
    }
    #[test]
    fn test_empty_string() {
        assert_eq!(sanitize_fts_query(""), "");
    }
    #[test]
    fn test_only_stop_words() {
        assert_eq!(sanitize_fts_query("the and for"), "");
    }
    #[test]
    fn test_domain_stop_words() {
        assert_eq!(sanitize_fts_query("board design circuit"), "");
    }
    #[test]
    fn test_single_char_dropped() {
        assert_eq!(sanitize_fts_query("a b c"), "");
    }
    #[test]
    fn test_quotes_stripped() {
        assert_eq!(sanitize_fts_query("\"hello\""), "\"hello\"*");
    }
    #[test]
    fn test_alias_expansion() {
        assert!(sanitize_fts_query("bluetooth").contains("BLE"));
    }
    #[test]
    fn test_synonym_eink() {
        let result = sanitize_fts_query("eink");
        assert!(result.contains("ink"));
        assert!(result.contains("paper"));
        assert!(result.contains("OR"));
    }
    #[test]
    fn test_synonym_e_paper() {
        let result = sanitize_fts_query("e-paper");
        assert!(result.contains("ink") && result.contains("paper") && result.contains("OR"));
    }
    #[test]
    fn test_alias_amplifier() {
        assert!(sanitize_fts_query("amplifier").contains("amp"));
    }
    #[test]
    fn test_power_not_stop_word() {
        let result = sanitize_fts_query("USB power delivery");
        assert!(result.contains("power"));
        assert!(result.contains("delivery"));
    }
    #[test]
    fn test_driver_not_stop_word() {
        let result = sanitize_fts_query("motor driver");
        assert!(result.contains("motor"));
        assert!(result.contains("driver"));
    }
    #[test]
    fn test_alias_esp32s3() {
        let result = sanitize_fts_query("esp32s3");
        assert!(result.contains("ESP32") && result.contains("S3"));
    }
    #[test]
    fn test_hyphenated_split() {
        let result = sanitize_fts_query("ESP32-S3");
        assert!(result.contains("ESP32") && result.contains("S3"));
    }
    #[test]
    fn test_mixed_terms_and_stop_words() {
        let result = sanitize_fts_query("the ESP32 board for WiFi");
        assert!(result.contains("ESP32"));
        assert!(result.contains("WiFi"));
        assert!(!result.contains("\"the\""));
        assert!(!result.contains("\"board\""));
    }

    // --- escape_like ---
    #[test]
    fn test_no_escaping() {
        assert_eq!(escape_like("MCP73831"), "MCP73831");
    }
    #[test]
    fn test_percent() {
        assert_eq!(escape_like("100%"), "100\\%");
    }
    #[test]
    fn test_underscore() {
        assert_eq!(escape_like("STM32_F4"), "STM32\\_F4");
    }
    #[test]
    fn test_backslash() {
        assert_eq!(escape_like("path\\to"), "path\\\\to");
    }

    // --- get_stats ---
    #[test]
    fn test_total_boards() {
        let conn = test_db();
        let stats = get_stats(&conn);
        assert_eq!(stats["total_boards"], 5);
    }
    #[test]
    fn test_formats() {
        let conn = test_db();
        let stats = get_stats(&conn);
        assert_eq!(stats["formats"]["kicad7"], 2);
        assert_eq!(stats["formats"]["eagle"], 3);
    }
    #[test]
    fn test_top_tags() {
        let conn = test_db();
        let stats = get_stats(&conn);
        assert!(stats["top_tags"].get("sensors").is_some());
        assert!(stats["top_tags"].get("battery-charging").is_some());
    }

    // --- search: free text ---
    #[test]
    fn test_basic_fts() {
        let conn = test_db();
        let r = search_boards(&conn, Some("ESP32"), None, None, None, None, 10);
        assert!(r.total >= 1);
        assert!(r.results.iter().any(|b| b.slug == "test-esp32-board"));
    }
    #[test]
    fn test_description_match() {
        let conn = test_db();
        let r = search_boards(&conn, Some("motor driver"), None, None, None, None, 10);
        assert!(r.total >= 1);
        assert!(r.results.iter().any(|b| b.slug == "adafruit-motor-shield"));
    }
    #[test]
    fn test_key_coverage_match() {
        let conn = test_db();
        let r = search_boards(&conn, Some("stepper"), None, None, None, None, 10);
        assert!(r.total >= 1);
    }
    #[test]
    fn test_no_results() {
        let conn = test_db();
        let r = search_boards(&conn, Some("zxynonexistent12345"), None, None, None, None, 10);
        assert_eq!(r.total, 0);
        assert!(r.results.is_empty());
    }
    #[test]
    fn test_empty_query_returns_all() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, None, None, None, 10);
        assert_eq!(r.total, 5);
    }
    #[test]
    fn test_empty_string_query_returns_all() {
        let conn = test_db();
        let r = search_boards(&conn, Some(""), None, None, None, None, 10);
        assert_eq!(r.total, 5);
    }
    #[test]
    fn test_only_stop_words_returns_all() {
        let conn = test_db();
        let r = search_boards(&conn, Some("the board design"), None, None, None, None, 10);
        assert_eq!(r.total, 5);
    }

    // --- search: component filter ---
    #[test]
    fn test_component_match() {
        let conn = test_db();
        let r = search_boards(&conn, None, Some("MCP73831"), None, None, None, 10);
        assert!(r.total >= 2);
        assert!(r.results.iter().any(|b| b.slug == "test-esp32-board"));
        assert!(r.results.iter().any(|b| b.slug == "sparkfun-mcp73831-charger"));
    }
    #[test]
    fn test_partial_component_match() {
        let conn = test_db();
        let r = search_boards(&conn, None, Some("MCP73"), None, None, None, 10);
        assert!(r.total >= 2);
    }
    #[test]
    fn test_empty_component_returns_all() {
        let conn = test_db();
        let r = search_boards(&conn, None, Some(""), None, None, None, 10);
        assert_eq!(r.total, 5);
    }
    #[test]
    fn test_nonexistent_component() {
        let conn = test_db();
        let r = search_boards(&conn, None, Some("NONEXISTENT999"), None, None, None, 10);
        assert_eq!(r.total, 0);
    }

    // --- search: tag filter ---
    #[test]
    fn test_single_tag() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, Some(&["sensors"]), None, None, 10);
        assert_eq!(r.total, 2);
        let slugs: HashSet<_> = r.results.iter().map(|b| b.slug.clone()).collect();
        assert!(slugs.contains("test-esp32-board"));
        assert!(slugs.contains("ble-sensor-node"));
    }
    #[test]
    fn test_multi_tag_and() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, Some(&["battery-charging", "sensors"]), None, None, 10);
        assert_eq!(r.total, 1);
        assert_eq!(r.results[0].slug, "test-esp32-board");
    }
    #[test]
    fn test_nonexistent_tag() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, Some(&["nonexistent-tag"]), None, None, 10);
        assert_eq!(r.total, 0);
    }
    #[test]
    fn test_tag_list_limit() {
        let conn = test_db();
        let many_tags: Vec<String> = (0..15).map(|i| format!("tag{i}")).collect();
        let refs: Vec<&str> = many_tags.iter().map(|s| s.as_str()).collect();
        let r = search_boards(&conn, None, None, Some(&refs), None, None, 10);
        assert_eq!(r.total, 0);
    }

    // --- search: org filter ---
    #[test]
    fn test_org_by_slug() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, None, Some("adafruit"), None, 10);
        assert_eq!(r.total, 1);
        assert_eq!(r.results[0].slug, "adafruit-motor-shield");
    }
    #[test]
    fn test_org_by_display_name() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, None, Some("Adafruit"), None, 10);
        assert_eq!(r.total, 1);
    }
    #[test]
    fn test_org_case_insensitive() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, None, Some("ADAFRUIT"), None, 10);
        assert_eq!(r.total, 1);
    }
    #[test]
    fn test_empty_org_returns_all() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, None, Some(""), None, 10);
        assert_eq!(r.total, 5);
    }
    #[test]
    fn test_org_soldered() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, None, Some("Soldered Electronics"), None, 10);
        assert_eq!(r.total, 1);
    }

    // --- search: layers filter ---
    #[test]
    fn test_4_layer() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, None, None, Some(4), 10);
        assert_eq!(r.total, 2);
    }
    #[test]
    fn test_2_layer() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, None, None, Some(2), 10);
        assert_eq!(r.total, 3);
    }
    #[test]
    fn test_nonexistent_layers() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, None, None, Some(8), 10);
        assert_eq!(r.total, 0);
    }

    // --- search: combined filters ---
    #[test]
    fn test_tag_and_org() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, Some(&["sensors"]), Some("Soldered Electronics"), None, 10);
        assert_eq!(r.total, 1);
        assert_eq!(r.results[0].slug, "ble-sensor-node");
    }
    #[test]
    fn test_query_and_tag() {
        let conn = test_db();
        let r = search_boards(&conn, Some("ESP32"), None, Some(&["battery-charging"]), None, None, 10);
        assert!(r.total >= 1);
        assert_eq!(r.results[0].slug, "test-esp32-board");
    }
    #[test]
    fn test_all_filters_no_match() {
        let conn = test_db();
        let r = search_boards(&conn, Some("ESP32"), None, Some(&["motor-control"]), None, None, 10);
        assert_eq!(r.total, 0);
    }

    // --- search: limit ---
    #[test]
    fn test_limit_respected() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, None, None, None, 2);
        assert_eq!(r.results.len(), 2);
        assert_eq!(r.total, 5);
    }
    #[test]
    fn test_limit_1() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, None, None, None, 1);
        assert_eq!(r.results.len(), 1);
    }

    // --- search: result shape ---
    #[test]
    fn test_org_and_org_display_differ() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, None, Some("Adafruit"), None, 1);
        let b = &r.results[0];
        assert_eq!(b.org.as_deref(), Some("adafruit"));
        assert_eq!(b.org_display.as_deref(), Some("Adafruit"));
    }
    #[test]
    fn test_source_url_format() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, None, None, None, 1);
        let b = &r.results[0];
        if b.source.is_some() {
            assert!(b.source_url.as_ref().unwrap().starts_with("https://github.com/"));
        }
    }
    #[test]
    fn test_matched_by_on_component_search() {
        let conn = test_db();
        let r = search_boards(&conn, None, Some("MCP73831"), None, None, None, 10);
        for b in &r.results {
            let hints = b.matched_by.as_ref().unwrap();
            assert!(hints.iter().any(|h| h.contains("MCP73831")));
        }
    }
    #[test]
    fn test_matched_by_on_fts_search() {
        let conn = test_db();
        let r = search_boards(&conn, Some("ESP32"), None, None, None, None, 10);
        for b in &r.results {
            let hints = b.matched_by.as_ref().unwrap();
            assert!(hints.contains(&"text match".to_string()));
        }
    }
    #[test]
    fn test_matched_by_on_tag_search() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, Some(&["sensors"]), None, None, 10);
        for b in &r.results {
            let hints = b.matched_by.as_ref().unwrap();
            assert!(hints.iter().any(|h| h.contains("sensors")));
        }
    }
    #[test]
    fn test_no_matched_by_on_unfiltered() {
        let conn = test_db();
        let r = search_boards(&conn, None, None, None, None, None, 10);
        for b in &r.results {
            assert!(b.matched_by.is_none());
        }
    }
    #[test]
    fn test_matched_by_key_ic_vs_component() {
        let conn = test_db();
        let r = search_boards(&conn, None, Some("ESP32-S3"), None, None, None, 10);
        let b = r.results.iter().find(|b| b.slug == "test-esp32-board").unwrap();
        assert!(b.matched_by.as_ref().unwrap().iter().any(|h| h.contains("key IC")));
    }
}
