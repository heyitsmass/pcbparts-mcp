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
}
