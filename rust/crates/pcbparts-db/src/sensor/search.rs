use rusqlite::{Connection, ToSql};
use std::collections::HashMap;

fn fts_stop_words() -> &'static [&'static str] {
    &[
        "a", "an", "the", "and", "or", "of", "for", "with", "in", "on", "to", "is", "it", "by",
        "at", "from", "as", "be", "my", "me", "do", "no", "so", "up", "if", "am", "are", "was",
        "has", "have", "had", "not", "but", "can", "will", "would", "could", "should", "what",
        "which", "that", "this", "these", "those", "how", "when", "where", "who", "than", "then",
        "also", "just", "very", "really", "any", "some", "about", "like", "into", "over", "such",
        "sensor", "sensors", "module", "modules", "board", "chip", "ic", "breakout", "give",
        "find", "show", "get", "list", "all", "best", "good", "recommend", "need", "want",
        "looking", "search", "use", "using", "used", "make", "work", "works", "detect",
        "measure", "monitor", "read", "reading",
    ]
}

pub fn sanitize_fts_query(query: &str) -> String {
    let clean = query.replace('"', "").replace('\'', "");
    let stop_words = fts_stop_words();
    let quoted: Vec<String> = clean
        .split_whitespace()
        .filter(|t| {
            let lower = t.to_lowercase();
            t.chars().count() >= 2 && !stop_words.contains(&lower.as_str())
        })
        .map(|t| format!("\"{}\"*", t))
        .collect();
    quoted.join(" AND ")
}

fn measure_expansions() -> HashMap<&'static str, Vec<&'static str>> {
    HashMap::from([("imu", vec!["acceleration", "gyroscope", "magnetic_field"])])
}

fn measure_query_aliases() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("barometric", "pressure"),
        ("altimeter", "pressure"),
        ("barometer", "pressure"),
        ("range finder", "distance"),
        ("rangefinder", "distance"),
        ("encoder", "rotation"),
        ("carbon monoxide", "co"),
        ("compass", "magnetic_field"),
        ("magnetometer", "magnetic_field"),
        ("accelerometer", "acceleration"),
        ("gyro", "gyroscope"),
        ("lux", "light"),
        ("ambient light", "light"),
        ("thermometer", "temperature"),
        ("hygrometer", "humidity"),
        ("air quality", "gas"),
        ("dust", "particulate"),
        ("pm2.5", "particulate"),
        ("pm10", "particulate"),
        ("sonar", "ultrasonic"),
    ])
}

pub fn protocol_aliases() -> HashMap<&'static str, Vec<&'static str>> {
    HashMap::from([("gpio", vec!["analog", "digital", "pwm", "one_wire"])])
}

#[derive(Debug, PartialEq)]
pub enum MeasureMode {
    Or,
    Single,
}

/// Resolve a single measure string to actual measure values and mode.
pub fn resolve_measure(measure: &str) -> (Vec<String>, MeasureMode) {
    let lower = measure.to_lowercase();
    let lower = lower.trim();

    if let Some(expansion) = measure_expansions().get(lower) {
        return (
            expansion.iter().map(|s| s.to_string()).collect(),
            MeasureMode::Or,
        );
    }
    if let Some(alias) = measure_query_aliases().get(lower) {
        return (vec![alias.to_string()], MeasureMode::Single);
    }
    (vec![lower.to_string()], MeasureMode::Single)
}

#[derive(Debug, Clone, PartialEq)]
pub struct SensorResult {
    pub id: String,
    pub name: String,
    pub manufacturer: Option<String>,
    pub sensor_type: Option<String>,
    pub voltage: Option<String>,
    pub datasheet_url: Option<String>,
    pub platform_count: i64,
    pub description: Option<String>,
    pub source_tier: String,
    pub measures: Vec<String>,
    pub protocols: Vec<String>,
    pub platforms: Vec<String>,
    pub urls: Vec<String>,
}

pub struct SearchSensorsResult {
    pub total: i64,
    pub results: Vec<SensorResult>,
}

/// Measure filter: mirrors Python's `str | list[str] | None`.
/// `Single` = one measure string (may resolve to an OR-expansion or an alias).
/// `And` = multiple measures — sensor must have ALL of them.
pub enum MeasureFilter<'a> {
    Single(&'a str),
    And(Vec<&'a str>),
}

#[allow(clippy::too_many_arguments)]
pub fn search_sensors(
    conn: &Connection,
    query: Option<&str>,
    measure: Option<MeasureFilter>,
    r#type: Option<&str>,
    protocol: Option<&str>,
    platform: Option<&str>,
    limit: i64,
    ic_aliases: &HashMap<String, String>,
) -> SearchSensorsResult {
    let mut where_clauses: Vec<String> = Vec::new();
    let mut joins: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    let mut multi_measure_count: Option<usize> = None;

    match measure {
        Some(MeasureFilter::Single(m)) => {
            let (resolved, mode) = resolve_measure(m);
            joins.push("JOIN sensor_measures sm ON sm.sensor_id = s.id".to_string());
            match mode {
                MeasureMode::Or => {
                    let placeholders = vec!["?"; resolved.len()].join(", ");
                    where_clauses.push(format!("sm.measure IN ({placeholders})"));
                    for r in resolved {
                        params.push(Box::new(r));
                    }
                }
                MeasureMode::Single => {
                    where_clauses.push("sm.measure = ?".to_string());
                    params.push(Box::new(resolved[0].clone()));
                }
            }
        }
        Some(MeasureFilter::And(measures)) if measures.len() == 1 => {
            let (resolved, mode) = resolve_measure(measures[0]);
            joins.push("JOIN sensor_measures sm ON sm.sensor_id = s.id".to_string());
            match mode {
                MeasureMode::Or => {
                    let placeholders = vec!["?"; resolved.len()].join(", ");
                    where_clauses.push(format!("sm.measure IN ({placeholders})"));
                    for r in resolved {
                        params.push(Box::new(r));
                    }
                }
                MeasureMode::Single => {
                    where_clauses.push("sm.measure = ?".to_string());
                    params.push(Box::new(resolved[0].clone()));
                }
            }
        }
        Some(MeasureFilter::And(measures)) => {
            let mut resolved_all: Vec<String> = Vec::new();
            for m in &measures {
                let (resolved, _) = resolve_measure(m);
                resolved_all.extend(resolved);
            }
            let n = resolved_all.len();
            let placeholders = vec!["?"; n].join(", ");
            joins.push(format!(
                "JOIN sensor_measures sm ON sm.sensor_id = s.id AND sm.measure IN ({placeholders})"
            ));
            for r in &resolved_all {
                params.push(Box::new(r.clone()));
            }
            where_clauses.push("1=1".to_string());
            multi_measure_count = Some(measures.len());
        }
        None => {}
    }

    if let Some(t) = r#type {
        where_clauses.push("s.type = ?".to_string());
        params.push(Box::new(t.to_lowercase().trim().to_string()));
    }

    if let Some(p) = protocol {
        let proto_lower = p.to_lowercase();
        let proto_lower = proto_lower.trim();
        if let Some(expanded) = protocol_aliases().get(proto_lower) {
            joins.push("JOIN sensor_protocols sp ON sp.sensor_id = s.id".to_string());
            let placeholders = vec!["?"; expanded.len()].join(", ");
            where_clauses.push(format!("sp.protocol IN ({placeholders})"));
            for e in expanded {
                params.push(Box::new(e.to_string()));
            }
        } else {
            joins.push("JOIN sensor_protocols sp ON sp.sensor_id = s.id".to_string());
            where_clauses.push("sp.protocol = ?".to_string());
            params.push(Box::new(proto_lower.to_string()));
        }
    }

    if let Some(pl) = platform {
        joins.push("JOIN sensor_platforms spl ON spl.sensor_id = s.id".to_string());
        where_clauses.push("spl.platform = ?".to_string());
        params.push(Box::new(pl.to_lowercase().trim().to_string()));
    }

    let mut order_boost = String::new();
    let mut order_boost_params: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(q) = query {
        let fts_query = sanitize_fts_query(q);
        if !fts_query.is_empty() {
            let normalized: String = q
                .to_lowercase()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect();
            let id_pattern = format!("%{normalized}%");
            let alias_target = ic_aliases.get(&normalized).cloned();

            if let Some(ref target) = alias_target {
                where_clauses.push(
                    "(s.id IN (SELECT id FROM sensors_fts WHERE sensors_fts MATCH ?) OR s.id LIKE ? OR s.id = ?)"
                        .to_string(),
                );
                params.push(Box::new(fts_query.clone()));
                params.push(Box::new(id_pattern.clone()));
                params.push(Box::new(target.clone()));

                order_boost = "CASE WHEN s.id = ? OR s.id = ? THEN 0 ELSE 1 END, ".to_string();
                order_boost_params.push(Box::new(normalized.clone()));
                order_boost_params.push(Box::new(target.clone()));
            } else {
                where_clauses.push(
                    "(s.id IN (SELECT id FROM sensors_fts WHERE sensors_fts MATCH ?) OR s.id LIKE ?)"
                        .to_string(),
                );
                params.push(Box::new(fts_query.clone()));
                params.push(Box::new(id_pattern.clone()));

                if !normalized.is_empty() {
                    order_boost = "CASE WHEN s.id = ? THEN 0 ELSE 1 END, ".to_string();
                    order_boost_params.push(Box::new(normalized.clone()));
                }
            }
        }
    }

    let join_sql = joins.join("\n    ");
    let where_sql = if where_clauses.is_empty() {
        "1=1".to_string()
    } else {
        where_clauses.join(" AND ")
    };
    let order_by = format!("ORDER BY {order_boost}s.platform_count DESC, s.name ASC");

    let select_cols = "s.id, s.name, s.manufacturer, s.type, s.voltage, s.datasheet_url, \
                       s.platform_count, s.description, s.source_tier";

    let (count_sql, data_sql, data_param_count_extra): (String, String, i64) =
        if let Some(n) = multi_measure_count {
            (
                format!(
                    "SELECT COUNT(*) FROM (SELECT s.id FROM sensors s {join_sql} WHERE {where_sql} \
                     GROUP BY s.id HAVING COUNT(DISTINCT sm.measure) = ?)"
                ),
                format!(
                    "SELECT {select_cols} FROM sensors s {join_sql} WHERE {where_sql} GROUP BY s.id \
                     HAVING COUNT(DISTINCT sm.measure) = ? {order_by} LIMIT ?"
                ),
                n as i64,
            )
        } else {
            (
                format!("SELECT COUNT(DISTINCT s.id) FROM sensors s {join_sql} WHERE {where_sql}"),
                format!(
                    "SELECT DISTINCT {select_cols} FROM sensors s {join_sql} WHERE {where_sql} \
                     {order_by} LIMIT ?"
                ),
                -1,
            )
        };

    let param_refs: Vec<&dyn ToSql> = params.iter().map(|b| b.as_ref()).collect();

    let mut count_params: Vec<&dyn ToSql> = param_refs.clone();
    let extra_count: Box<dyn ToSql>;
    if data_param_count_extra >= 0 {
        extra_count = Box::new(data_param_count_extra);
        count_params.push(extra_count.as_ref());
    }
    let total: i64 = conn
        .query_row(&count_sql, count_params.as_slice(), |r| r.get(0))
        .unwrap_or(0);

    let mut data_params: Vec<&dyn ToSql> = param_refs.clone();
    let extra_data: Box<dyn ToSql>;
    if data_param_count_extra >= 0 {
        extra_data = Box::new(data_param_count_extra);
        data_params.push(extra_data.as_ref());
    }
    let boost_refs: Vec<&dyn ToSql> = order_boost_params.iter().map(|b| b.as_ref()).collect();
    data_params.extend(boost_refs);
    let limit_box: Box<dyn ToSql> = Box::new(limit);
    data_params.push(limit_box.as_ref());

    let mut stmt = conn.prepare(&data_sql).unwrap();
    let ids: Vec<String> = stmt
        .query_map(data_params.as_slice(), |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    // Re-run to get full rows (query_map above only grabbed id; instead map full row directly)
    let mut stmt2 = conn.prepare(&data_sql).unwrap();
    let rows: Vec<(
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        i64,
        Option<String>,
        String,
    )> = stmt2
        .query_map(data_params.as_slice(), |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    debug_assert_eq!(ids.len(), rows.len());

    let mut results = Vec::with_capacity(rows.len());
    for (id, name, manufacturer, sensor_type, voltage, datasheet_url, platform_count, description, source_tier) in rows {
        let measures = fetch_col(conn, "sensor_measures", "measure", &id);
        let protocols = fetch_col(conn, "sensor_protocols", "protocol", &id);
        let platforms = fetch_col(conn, "sensor_platforms", "platform", &id);
        let urls = fetch_col(conn, "sensor_urls", "url", &id);
        results.push(SensorResult {
            id,
            name,
            manufacturer,
            sensor_type,
            voltage,
            datasheet_url,
            platform_count,
            description,
            source_tier,
            measures,
            protocols,
            platforms,
            urls,
        });
    }

    SearchSensorsResult { total, results }
}

fn fetch_col(conn: &Connection, table: &str, col: &str, sensor_id: &str) -> Vec<String> {
    let sql = format!("SELECT {col} FROM {table} WHERE sensor_id = ? ORDER BY {col}");
    let mut stmt = conn.prepare(&sql).unwrap();
    stmt.query_map([sensor_id], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use crate::sensor::SCHEMA;

    struct Fixture {
        id: &'static str,
        name: &'static str,
        manufacturer: Option<&'static str>,
        sensor_type: Option<&'static str>,
        voltage: Option<&'static str>,
        platform_count: i64,
        description: &'static str,
        source_tier: &'static str,
        measures: &'static [&'static str],
        protocols: &'static [&'static str],
        platforms: &'static [&'static str],
        urls: &'static [&'static str],
    }

    fn fixtures() -> Vec<Fixture> {
        vec![
            Fixture { id: "bme280", name: "BME280", manufacturer: Some("Bosch"), sensor_type: Some("mems"), voltage: Some("1.8-3.6"), platform_count: 7, description: "Combined humidity, pressure, and temperature sensor", source_tier: "primary", measures: &["humidity", "pressure", "temperature"], protocols: &["i2c", "spi"], platforms: &["arduino", "circuitpython", "esphome", "micropython", "tasmota", "zephyr", "raspberry_pi"], urls: &["https://esphome.io/components/sensor/bme280"] },
            Fixture { id: "scd4x", name: "SCD4X", manufacturer: Some("Sensirion"), sensor_type: Some("photoacoustic"), voltage: Some("2.4-5.5"), platform_count: 7, description: "CO2 humidity and temperature sensor", source_tier: "primary", measures: &["co2", "humidity", "temperature"], protocols: &["i2c"], platforms: &["arduino", "circuitpython", "esphome", "micropython", "tasmota", "zephyr", "raspberry_pi"], urls: &[] },
            Fixture { id: "mpu6050", name: "MPU6050", manufacturer: Some("TDK"), sensor_type: Some("mems"), voltage: Some("2.375-3.46"), platform_count: 6, description: "Six-axis accelerometer and gyroscope MEMS IMU sensor", source_tier: "primary", measures: &["acceleration", "gyroscope", "temperature"], protocols: &["i2c"], platforms: &["arduino", "circuitpython", "esphome", "micropython", "tasmota", "zephyr"], urls: &[] },
            Fixture { id: "ds18b20", name: "DS18B20", manufacturer: Some("Analog Devices"), sensor_type: None, voltage: Some("3.0-5.5"), platform_count: 5, description: "Waterproof digital temperature sensor one wire", source_tier: "primary", measures: &["temperature"], protocols: &["one_wire"], platforms: &["arduino", "esphome", "micropython", "tasmota", "zephyr"], urls: &[] },
            Fixture { id: "mhz19", name: "MHZ19", manufacturer: Some("Winsen"), sensor_type: Some("ndir"), voltage: Some("4.5-5.5"), platform_count: 4, description: "NDIR CO2 gas sensor module with UART interface", source_tier: "primary", measures: &["co2", "gas", "temperature"], protocols: &["uart"], platforms: &["arduino", "esphome", "micropython", "tasmota"], urls: &[] },
            Fixture { id: "vl53l0x", name: "VL53L0X", manufacturer: Some("STMicroelectronics"), sensor_type: Some("tof"), voltage: Some("2.6-3.5"), platform_count: 5, description: "Time of flight distance ranging sensor", source_tier: "primary", measures: &["distance"], protocols: &["i2c"], platforms: &["arduino", "circuitpython", "esphome", "micropython", "zephyr"], urls: &[] },
            Fixture { id: "breakout1", name: "BREAKOUT1", manufacturer: None, sensor_type: None, voltage: None, platform_count: 0, description: "SparkFun only breakout sensor for light detection", source_tier: "breakout_only", measures: &["light"], protocols: &["analog"], platforms: &[], urls: &[] },
        ]
    }

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        for f in fixtures() {
            conn.execute(
                "INSERT INTO sensors (id, name, manufacturer, type, voltage, datasheet_url, platform_count, description, source_tier, sources) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?8, '[\"test\"]')",
                rusqlite::params![f.id, f.name, f.manufacturer, f.sensor_type, f.voltage, f.platform_count, f.description, f.source_tier],
            ).unwrap();
            conn.execute(
                "INSERT INTO sensors_fts (id, name, manufacturer, description) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![f.id, f.name, f.manufacturer.unwrap_or(""), f.description],
            ).unwrap();
            for m in f.measures {
                conn.execute("INSERT INTO sensor_measures VALUES (?1, ?2)", rusqlite::params![f.id, m]).unwrap();
            }
            for p in f.protocols {
                conn.execute("INSERT INTO sensor_protocols VALUES (?1, ?2)", rusqlite::params![f.id, p]).unwrap();
            }
            for pl in f.platforms {
                conn.execute("INSERT INTO sensor_platforms VALUES (?1, ?2)", rusqlite::params![f.id, pl]).unwrap();
            }
            for u in f.urls {
                conn.execute("INSERT INTO sensor_urls VALUES (?1, ?2)", rusqlite::params![f.id, u]).unwrap();
            }
        }
        conn
    }

    fn no_aliases() -> HashMap<String, String> {
        HashMap::new()
    }

    // --- search_sensors: FTS ---
    #[test]
    fn test_fts_by_name() {
        let conn = test_db();
        let r = search_sensors(&conn, Some("BME280"), None, None, None, None, 15, &no_aliases());
        assert!(r.total >= 1);
        assert_eq!(r.results[0].id, "bme280");
    }
    #[test]
    fn test_fts_by_description() {
        let conn = test_db();
        let r = search_sensors(&conn, Some("waterproof"), None, None, None, None, 15, &no_aliases());
        assert!(r.total >= 1);
        assert_eq!(r.results[0].id, "ds18b20");
    }
    #[test]
    fn test_fts_by_manufacturer() {
        let conn = test_db();
        let r = search_sensors(&conn, Some("Bosch"), None, None, None, None, 15, &no_aliases());
        assert!(r.total >= 1);
        assert!(r.results.iter().any(|s| s.id == "bme280"));
    }
    #[test]
    fn test_fts_no_results() {
        let conn = test_db();
        let r = search_sensors(&conn, Some("nonexistentsensor12345"), None, None, None, None, 15, &no_aliases());
        assert_eq!(r.total, 0);
        assert!(r.results.is_empty());
    }
    #[test]
    fn test_fts_natural_language() {
        let conn = test_db();
        let r = search_sensors(&conn, Some("find a good temperature and humidity sensor"), None, None, None, None, 15, &no_aliases());
        assert!(r.total >= 1);
        assert!(r.results.iter().any(|s| s.id == "bme280"));
    }
    #[test]
    fn test_fts_prefix_match() {
        let conn = test_db();
        let r = search_sensors(&conn, Some("BME"), None, None, None, None, 15, &no_aliases());
        assert!(r.total >= 1);
        assert!(r.results.iter().any(|s| s.id == "bme280"));
    }
    #[test]
    fn test_fts_prefix_partial_model() {
        let conn = test_db();
        let r = search_sensors(&conn, Some("VL53"), None, None, None, None, 15, &no_aliases());
        assert!(r.total >= 1);
        assert!(r.results.iter().any(|s| s.id == "vl53l0x"));
    }
    #[test]
    fn test_id_like_fallback() {
        let conn = test_db();
        let r = search_sensors(&conn, Some("mhz"), None, None, None, None, 15, &no_aliases());
        assert!(r.total >= 1);
        assert!(r.results.iter().any(|s| s.id == "mhz19"));
    }

    // --- search_sensors: measure ---
    #[test]
    fn test_single_measure() {
        let conn = test_db();
        let r = search_sensors(&conn, None, Some(MeasureFilter::Single("co2")), None, None, None, 15, &no_aliases());
        assert!(r.total >= 2);
        let ids: std::collections::HashSet<_> = r.results.iter().map(|s| s.id.clone()).collect();
        assert!(ids.contains("scd4x"));
        assert!(ids.contains("mhz19"));
    }
    #[test]
    fn test_multi_measure_and() {
        let conn = test_db();
        let r = search_sensors(&conn, None, Some(MeasureFilter::And(vec!["temperature", "pressure"])), None, None, None, 15, &no_aliases());
        assert!(r.total >= 1);
        assert!(r.results.iter().any(|s| s.id == "bme280"));
        assert!(r.results.iter().all(|s| s.id != "ds18b20"));
    }
    #[test]
    fn test_imu_expansion_search() {
        let conn = test_db();
        let r = search_sensors(&conn, None, Some(MeasureFilter::Single("imu")), None, None, None, 15, &no_aliases());
        assert!(r.total >= 1);
        assert!(r.results.iter().any(|s| s.id == "mpu6050"));
    }
    #[test]
    fn test_measure_alias() {
        let conn = test_db();
        let r = search_sensors(&conn, None, Some(MeasureFilter::Single("barometric")), None, None, None, 15, &no_aliases());
        assert!(r.total >= 1);
        assert!(r.results.iter().any(|s| s.id == "bme280"));
    }
    #[test]
    fn test_list_single_item() {
        let conn = test_db();
        let r = search_sensors(&conn, None, Some(MeasureFilter::And(vec!["distance"])), None, None, None, 15, &no_aliases());
        assert!(r.total >= 1);
        assert!(r.results.iter().any(|s| s.id == "vl53l0x"));
    }

    // --- search_sensors: protocol ---
    #[test]
    fn test_i2c() {
        let conn = test_db();
        let r = search_sensors(&conn, None, None, None, Some("i2c"), None, 15, &no_aliases());
        let ids: std::collections::HashSet<_> = r.results.iter().map(|s| s.id.clone()).collect();
        assert!(ids.contains("bme280"));
        assert!(ids.contains("scd4x"));
        assert!(!ids.contains("mhz19"));
    }
    #[test]
    fn test_uart() {
        let conn = test_db();
        let r = search_sensors(&conn, None, None, None, Some("uart"), None, 15, &no_aliases());
        let ids: std::collections::HashSet<_> = r.results.iter().map(|s| s.id.clone()).collect();
        assert!(ids.contains("mhz19"));
        assert!(!ids.contains("bme280"));
    }
    #[test]
    fn test_gpio_expansion() {
        let conn = test_db();
        let r = search_sensors(&conn, None, None, None, Some("gpio"), None, 15, &no_aliases());
        let ids: std::collections::HashSet<_> = r.results.iter().map(|s| s.id.clone()).collect();
        assert!(ids.contains("ds18b20"));
        assert!(ids.contains("breakout1"));
    }

    // --- search_sensors: platform ---
    #[test]
    fn test_filter_platform() {
        let conn = test_db();
        let r = search_sensors(&conn, None, None, None, None, Some("circuitpython"), 15, &no_aliases());
        let ids: std::collections::HashSet<_> = r.results.iter().map(|s| s.id.clone()).collect();
        assert!(ids.contains("bme280"));
        assert!(!ids.contains("mhz19"));
    }
    #[test]
    fn test_zephyr() {
        let conn = test_db();
        let r = search_sensors(&conn, None, None, None, None, Some("zephyr"), 15, &no_aliases());
        assert!(r.total >= 1);
        assert!(r.results.iter().any(|s| s.id == "bme280"));
    }

    // --- search_sensors: type ---
    #[test]
    fn test_filter_type() {
        let conn = test_db();
        let r = search_sensors(&conn, None, None, Some("ndir"), None, None, 15, &no_aliases());
        assert!(r.total >= 1);
        assert_eq!(r.results[0].id, "mhz19");
    }
    #[test]
    fn test_tof() {
        let conn = test_db();
        let r = search_sensors(&conn, None, None, Some("tof"), None, None, 15, &no_aliases());
        assert!(r.total >= 1);
        assert!(r.results.iter().any(|s| s.id == "vl53l0x"));
    }
    #[test]
    fn test_mems() {
        let conn = test_db();
        let r = search_sensors(&conn, None, None, Some("mems"), None, None, 15, &no_aliases());
        let ids: std::collections::HashSet<_> = r.results.iter().map(|s| s.id.clone()).collect();
        assert!(ids.contains("bme280"));
        assert!(ids.contains("mpu6050"));
    }

    // --- search_sensors: combined ---
    #[test]
    fn test_measure_plus_protocol() {
        let conn = test_db();
        let r = search_sensors(&conn, None, Some(MeasureFilter::Single("co2")), None, Some("uart"), None, 15, &no_aliases());
        let ids: std::collections::HashSet<_> = r.results.iter().map(|s| s.id.clone()).collect();
        assert!(ids.contains("mhz19"));
        assert!(!ids.contains("scd4x"));
    }
    #[test]
    fn test_measure_plus_platform() {
        let conn = test_db();
        let r = search_sensors(&conn, None, Some(MeasureFilter::Single("temperature")), None, None, Some("circuitpython"), 15, &no_aliases());
        let ids: std::collections::HashSet<_> = r.results.iter().map(|s| s.id.clone()).collect();
        assert!(ids.contains("bme280"));
        assert!(!ids.contains("ds18b20"));
    }
    #[test]
    fn test_fts_plus_measure() {
        let conn = test_db();
        let r = search_sensors(&conn, Some("waterproof"), Some(MeasureFilter::Single("temperature")), None, None, None, 15, &no_aliases());
        assert!(r.total >= 1);
        assert_eq!(r.results[0].id, "ds18b20");
    }
    #[test]
    fn test_all_filters() {
        let conn = test_db();
        let r = search_sensors(&conn, None, Some(MeasureFilter::Single("temperature")), Some("mems"), Some("i2c"), Some("arduino"), 15, &no_aliases());
        assert!(r.results.iter().any(|s| s.id == "bme280"));
    }

    // --- search_sensors: results ---
    #[test]
    fn test_result_structure() {
        let conn = test_db();
        let r = search_sensors(&conn, Some("BME280"), None, None, None, None, 15, &no_aliases());
        let s = &r.results[0];
        assert_eq!(s.id, "bme280");
        assert_eq!(s.name, "BME280");
        assert_eq!(s.manufacturer.as_deref(), Some("Bosch"));
        assert_eq!(s.sensor_type.as_deref(), Some("mems"));
        assert_eq!(s.voltage.as_deref(), Some("1.8-3.6"));
        let mut measures = s.measures.clone();
        measures.sort();
        assert_eq!(measures, vec!["humidity", "pressure", "temperature"]);
        let mut protocols = s.protocols.clone();
        protocols.sort();
        assert_eq!(protocols, vec!["i2c", "spi"]);
        assert!(s.platforms.contains(&"arduino".to_string()));
        assert_eq!(s.platform_count, 7);
        assert_eq!(s.source_tier, "primary");
    }
    #[test]
    fn test_sort_by_platform_count() {
        let conn = test_db();
        let r = search_sensors(&conn, None, Some(MeasureFilter::Single("temperature")), None, None, None, 15, &no_aliases());
        let counts: Vec<i64> = r.results.iter().map(|s| s.platform_count).collect();
        let mut sorted = counts.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(counts, sorted);
    }
    #[test]
    fn test_limit() {
        let conn = test_db();
        let r = search_sensors(&conn, None, Some(MeasureFilter::Single("temperature")), None, None, None, 2, &no_aliases());
        assert!(r.results.len() <= 2);
        assert!(r.total > 2);
    }
    #[test]
    fn test_no_params_returns_nothing() {
        let conn = test_db();
        let r = search_sensors(&conn, None, None, None, None, None, 15, &no_aliases());
        assert_eq!(r.total, fixtures().len() as i64);
    }

    // --- sanitize_fts_query ---
    #[test]
    fn test_basic_query() {
        assert_eq!(sanitize_fts_query("BME280"), "\"BME280\"*");
    }
    #[test]
    fn test_multi_term() {
        assert_eq!(sanitize_fts_query("temperature humidity"), "\"temperature\"* AND \"humidity\"*");
    }
    #[test]
    fn test_stop_words_filtered() {
        assert_eq!(sanitize_fts_query("temperature and humidity"), "\"temperature\"* AND \"humidity\"*");
        assert_eq!(sanitize_fts_query("give me all temperature sensors"), "\"temperature\"*");
    }
    #[test]
    fn test_strips_quotes() {
        assert_eq!(sanitize_fts_query("\"test\""), "\"test\"*");
    }
    #[test]
    fn test_skips_short_terms() {
        assert_eq!(sanitize_fts_query("a temperature"), "\"temperature\"*");
    }
    #[test]
    fn test_empty() {
        assert_eq!(sanitize_fts_query(""), "");
    }
    #[test]
    fn test_prefix_match_format() {
        assert_eq!(sanitize_fts_query("BM22S"), "\"BM22S\"*");
    }

    // --- resolve_measure ---
    #[test]
    fn test_imu_expansion() {
        let (measures, mode) = resolve_measure("imu");
        assert_eq!(mode, MeasureMode::Or);
        let set: std::collections::HashSet<_> = measures.into_iter().collect();
        assert_eq!(set, ["acceleration", "gyroscope", "magnetic_field"].iter().map(|s| s.to_string()).collect());
    }
    #[test]
    fn test_voc_passthrough() {
        let (measures, mode) = resolve_measure("voc");
        assert_eq!(mode, MeasureMode::Single);
        assert_eq!(measures, vec!["voc".to_string()]);
    }
    #[test]
    fn test_alias_barometric() {
        let (measures, _) = resolve_measure("barometric");
        assert_eq!(measures, vec!["pressure".to_string()]);
    }
    #[test]
    fn test_passthrough() {
        let (measures, mode) = resolve_measure("co2");
        assert_eq!(mode, MeasureMode::Single);
        assert_eq!(measures, vec!["co2".to_string()]);
    }
    #[test]
    fn test_case_insensitive() {
        let (measures, _) = resolve_measure("IMU");
        let set: std::collections::HashSet<_> = measures.into_iter().collect();
        assert_eq!(set, ["acceleration", "gyroscope", "magnetic_field"].iter().map(|s| s.to_string()).collect());
    }
}
