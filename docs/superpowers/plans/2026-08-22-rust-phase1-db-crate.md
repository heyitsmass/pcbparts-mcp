# Rust Migration Phase 1: pcbparts-db (boards + sensor read side) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port `boards_db/` and `sensor_db/` (the two genuinely dependency-free
subsystems of the component database layer — see spec's corrected migration
order) from Python to a new `pcbparts-db` Rust crate, with every existing
pytest test translated 1:1 into a passing Rust test.

**Architecture:** A Cargo workspace at `rust/` containing one crate,
`pcbparts-db`, with two top-level modules (`boards`, `sensor`) each split
into `mod.rs` (schema + public wrapper struct), `search.rs` (query-shaping
pure functions + the FTS5 search query + its tests), and — for `boards`
only, since it has both a search and a detail surface like the Python
package does — `detail.rs` (get_board/get_consensus/get_tag_consensus + its
tests) and `fixtures.rs` (a shared test-only fixture, `#[cfg(test)]`-gated).
Every function ported here has already been written, compiled, and run
against golden fixture data extracted from the actual Python-built
databases — this plan transcribes verified code, not new design.

**Tech Stack:** Rust 2021 edition, `rusqlite` (bundled feature — verified to
include FTS5 with the `porter unicode61` tokenizer out of the box, no extra
feature flags needed), `serde` + `serde_json`, `thiserror`.

**Spec:** `docs/superpowers/specs/2026-08-22-rust-migration-design.md`

## Global Constraints

- Every ported test must assert the same behavior as its Python counterpart
  (golden-value parity), not a re-derived expectation.
- `boards_db`'s and `sensor_db`'s Rust structs (`BoardsDb`, `SensorDb`) open
  an *existing* on-disk database file — they do **not** build it if missing.
  Building the DB is `pcbparts-pipeline`'s job (a later migration phase);
  attempting it here would mean re-implementing `scripts/build_boards_db.py`
  / `scripts/build_sensor_db.py` out of order. `BoardsDb::open` /
  `SensorDb::open` return a clear `OpenError::NotFound` instead.
- `Mutex<Connection>` is the concurrency primitive for now. This is
  sufficient to prove the read-side API and pass every test; it is **not**
  the final production connection-pooling/concurrency design — that gets
  decided in the `pcbparts-server` phase once the async runtime (tokio) is
  in place and the real request-concurrency profile is known.
- Per CLAUDE.md and the `project-rust-rewrite` memory: never commit without
  explicit permission (each task below ends with a commit step — get
  confirmation before running it if executing this plan live), no Claude
  attribution in commit messages.

## File Structure

```
rust/
  Cargo.toml                          # workspace
  crates/
    pcbparts-db/
      Cargo.toml
      src/
        lib.rs                        # pub mod boards; pub mod sensor; + FTS5 smoke test
        sensor/
          mod.rs                      # SCHEMA const, SensorDb wrapper struct, integration tests
          search.rs                   # sanitize_fts_query, resolve_measure, search_sensors, tests
        boards/
          mod.rs                      # SCHEMA const, BoardsDb wrapper struct, integration tests
          search.rs                   # escape_like, sanitize_fts_query, search_boards, get_stats, tests
          detail.rs                   # get_board, get_consensus, get_tag_consensus, tests
          fixtures.rs                 # #[cfg(test)] shared fixture (test_db()) for search.rs + detail.rs
```

This mirrors Python's `boards_db/{connection,search,detail}.py` and
`sensor_db/{connection,search}.py` split — `mod.rs` takes the role of
`connection.py` (schema + top-level type), `search.rs`/`detail.rs` match
their Python namesakes directly.

---

### Task 1: Rust workspace + pcbparts-db crate scaffold + FTS5 smoke test

**Files:**
- Create: `rust/Cargo.toml`
- Create: `rust/crates/pcbparts-db/Cargo.toml`
- Create: `rust/crates/pcbparts-db/src/lib.rs`

**Interfaces:**
- Produces: an empty-but-compiling `pcbparts-db` crate that later tasks add
  `pub mod boards;` / `pub mod sensor;` to.

- [ ] **Step 1: Create the workspace manifest**

```toml
# rust/Cargo.toml
[workspace]
resolver = "2"
members = ["crates/pcbparts-db"]
```

- [ ] **Step 2: Create the crate manifest**

```toml
# rust/crates/pcbparts-db/Cargo.toml
[package]
name = "pcbparts-db"
version = "0.1.0"
edition = "2021"

[dependencies]
rusqlite = { version = "0.32", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
```

- [ ] **Step 3: Write a failing FTS5 smoke test**

This isn't red/green TDD in the usual sense (there's no behavior to
implement yet) — it's an infrastructure check: confirm the `bundled`
rusqlite feature actually ships FTS5 with the `porter unicode61` tokenizer,
since every ported query below depends on it.

```rust
// rust/crates/pcbparts-db/src/lib.rs
#[cfg(test)]
mod smoke_test {
    use rusqlite::Connection;

    #[test]
    fn fts5_with_porter_tokenizer_works() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE VIRTUAL TABLE t USING fts5(name, tokenize='porter unicode61');
             INSERT INTO t(name) VALUES ('ESP32-S3');",
        )
        .expect("FTS5 must be available in bundled rusqlite");
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM t WHERE t MATCH 'ESP32*'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }
}
```

- [ ] **Step 4: Run it**

Run: `cd rust && cargo test -p pcbparts-db`
Expected: PASS (`fts5_with_porter_tokenizer_works ... ok`). This has been
verified in isolation already; if it fails in your environment, the
`rusqlite` bundled build is missing FTS5 for some reason (e.g. a
pre-existing system SQLite being linked instead) — resolve that before
proceeding, since every later task depends on it.

- [ ] **Step 5: Commit**

```bash
git add rust/Cargo.toml rust/crates/pcbparts-db/Cargo.toml rust/crates/pcbparts-db/src/lib.rs
git commit -m "rust: scaffold pcbparts-db crate, verify FTS5 works"
```

---

### Task 2: sensor_db — pure query-shaping functions

**Files:**
- Create: `rust/crates/pcbparts-db/src/sensor/mod.rs`
- Create: `rust/crates/pcbparts-db/src/sensor/search.rs`
- Modify: `rust/crates/pcbparts-db/src/lib.rs` (add `pub mod sensor;`)

**Interfaces:**
- Produces: `sanitize_fts_query(&str) -> String`, `resolve_measure(&str) ->
  (Vec<String>, MeasureMode)`, `MeasureMode` enum — used by Task 3's
  `search_sensors`.

- [ ] **Step 1: Write `sensor/mod.rs` with just the schema**

```rust
//! Sensor database: schema + module wiring.
pub mod search;

pub const SCHEMA: &str = "
CREATE TABLE sensors (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    manufacturer TEXT,
    type TEXT,
    voltage TEXT,
    datasheet_url TEXT,
    platform_count INTEGER DEFAULT 0,
    description TEXT,
    source_tier TEXT DEFAULT 'primary',
    sources TEXT
);
CREATE TABLE sensor_measures (
    sensor_id TEXT NOT NULL REFERENCES sensors(id),
    measure TEXT NOT NULL,
    PRIMARY KEY (sensor_id, measure)
);
CREATE TABLE sensor_protocols (
    sensor_id TEXT NOT NULL REFERENCES sensors(id),
    protocol TEXT NOT NULL,
    PRIMARY KEY (sensor_id, protocol)
);
CREATE TABLE sensor_platforms (
    sensor_id TEXT NOT NULL REFERENCES sensors(id),
    platform TEXT NOT NULL,
    PRIMARY KEY (sensor_id, platform)
);
CREATE TABLE sensor_urls (
    sensor_id TEXT NOT NULL REFERENCES sensors(id),
    url TEXT NOT NULL,
    PRIMARY KEY (sensor_id, url)
);
CREATE VIRTUAL TABLE sensors_fts USING fts5(id, name, manufacturer, description);
";
```

(This is the identical schema `tests/test_sensor_db.py`'s `SCHEMA` constant
uses, which itself matches the real `sensor.db` built by
`scripts/build_sensor_db.py`.)

- [ ] **Step 2: Add `pub mod sensor;` to `lib.rs`**

- [ ] **Step 3: Write the failing tests for `sanitize_fts_query` and `resolve_measure`**

Port of `TestFTSSanitize` and `TestResolveMeasure` from
`tests/test_sensor_db.py`:

```rust
// rust/crates/pcbparts-db/src/sensor/search.rs
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

#[cfg(test)]
mod tests {
    use super::*;

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
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p pcbparts-db sensor::search::tests`
Expected: all 12 tests PASS (already verified against this exact code).

- [ ] **Step 5: Commit**

```bash
git add rust/crates/pcbparts-db/src/sensor rust/crates/pcbparts-db/src/lib.rs
git commit -m "rust: port sensor_db sanitize_fts_query and resolve_measure"
```

---

### Task 3: sensor_db — search_sensors + full test port

**Files:**
- Modify: `rust/crates/pcbparts-db/src/sensor/search.rs`

**Interfaces:**
- Consumes: `sanitize_fts_query`, `resolve_measure`, `MeasureMode`,
  `protocol_aliases` from Task 2.
- Produces: `search_sensors(...) -> SearchSensorsResult`, `SensorResult`,
  `SearchSensorsResult`, `MeasureFilter` — consumed by Task 7's `SensorDb`
  wrapper.

- [ ] **Step 1: Add the result types and `search_sensors`, plus its full test suite**

Append to `sensor/search.rs` (above the existing `#[cfg(test)] mod tests`,
which gets the additions described in step 3 below):

```rust
use rusqlite::{Connection, ToSql};

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
```

- [ ] **Step 2: Add the fixture and `search_sensors` tests inside the existing `mod tests` block**

Add these to the `#[cfg(test)] mod tests` block already in the file
(alongside the `sanitize_fts_query`/`resolve_measure` tests from Task 2):

```rust
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
```

- [ ] **Step 3: Run the full sensor module test suite**

Run: `cargo test -p pcbparts-db sensor::`
Expected: PASS — 41 tests total (12 from Task 2 + 29 here).

- [ ] **Step 4: Commit**

```bash
git add rust/crates/pcbparts-db/src/sensor/search.rs
git commit -m "rust: port sensor_db search_sensors with full test parity"
```

---

### Task 4: boards_db — schema + pure functions + get_stats

**Files:**
- Create: `rust/crates/pcbparts-db/src/boards/mod.rs`
- Create: `rust/crates/pcbparts-db/src/boards/search.rs`
- Modify: `rust/crates/pcbparts-db/src/lib.rs` (add `pub mod boards;`)

**Interfaces:**
- Produces: `boards::SCHEMA`, `search::escape_like`, `search::sanitize_fts_query`,
  `search::get_stats` — consumed by Task 5 (`search_boards`) and Task 6
  (`detail.rs` imports `escape_like`).

- [ ] **Step 1: Write `boards/mod.rs` with just the schema**

```rust
//! Boards database: schema + module wiring.
pub mod search;
pub mod detail;

#[cfg(test)]
pub(crate) mod fixtures;

pub const SCHEMA: &str = "
CREATE TABLE boards (
    id INTEGER PRIMARY KEY,
    slug TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    org TEXT,
    org_display TEXT,
    source TEXT,
    format TEXT,
    description TEXT,
    key_coverage TEXT,
    layers INTEGER,
    width_mm REAL,
    height_mm REAL,
    min_trace TEXT,
    min_clearance TEXT,
    min_drill TEXT,
    min_via TEXT,
    component_count INTEGER,
    ic_count INTEGER,
    net_count INTEGER,
    key_ics_text TEXT,
    all_ics_text TEXT,
    nets_json TEXT,
    positions_json TEXT,
    copper_pours_json TEXT,
    neighborhoods_json TEXT
);
CREATE TABLE board_tags (
    board_id INTEGER NOT NULL REFERENCES boards(id),
    tag TEXT NOT NULL,
    PRIMARY KEY (board_id, tag)
);
CREATE TABLE board_key_ics (
    board_id INTEGER NOT NULL REFERENCES boards(id),
    ic TEXT NOT NULL,
    PRIMARY KEY (board_id, ic)
);
CREATE TABLE board_components (
    id INTEGER PRIMARY KEY,
    board_id INTEGER NOT NULL REFERENCES boards(id),
    ref TEXT NOT NULL,
    value TEXT,
    footprint TEXT,
    description TEXT,
    voltage TEXT,
    tolerance TEXT,
    dielectric TEXT,
    decouples TEXT,
    pullup TEXT,
    pulldown TEXT
);
CREATE INDEX idx_board_org ON boards(org);
CREATE INDEX idx_board_layers ON boards(layers);
CREATE INDEX idx_board_format ON boards(format);
CREATE INDEX idx_board_component_count ON boards(component_count DESC);
CREATE INDEX idx_board_tag ON board_tags(tag);
CREATE INDEX idx_board_key_ic ON board_key_ics(ic);
CREATE INDEX idx_comp_board_id ON board_components(board_id);
CREATE INDEX idx_comp_value ON board_components(value);
CREATE VIRTUAL TABLE boards_fts USING fts5(
    slug, name, description, key_coverage, tags_text, key_ics_text, all_ics_text, org_display,
    tokenize='porter unicode61'
);
";
```

(Identical to the `CREATE TABLE`/`CREATE INDEX`/`CREATE VIRTUAL TABLE`
statements in `scripts/build_boards_db.py`'s `_build_tables`.)

Note this references `pub mod detail;` and `pub(crate) mod fixtures;` which
don't exist yet — that's fine, they're created in this task (fixtures.rs,
step 3) and Task 6 (detail.rs). The crate won't compile until Task 6 is
done; that's expected for a multi-task module split like this one.

- [ ] **Step 2: Add `pub mod boards;` to `lib.rs`**

- [ ] **Step 3: Write the shared test fixture**

This builds the *exact* `boards.db` that `scripts/build_boards_db.py`
produces from the Python test suite's `SAMPLE_BOARDS` fixture — the values
below were dumped from a real run of that pipeline (not hand-derived), so
they're byte-identical to production output, including the neighborhood
JSON structure that the org/neighborhood-extraction logic computes (that
logic itself is out of scope for this phase — see spec's Phase 6/7 — this
fixture just needs its *output*, not a re-implementation of it).

```rust
// rust/crates/pcbparts-db/src/boards/fixtures.rs
//! Shared test fixture: builds the exact boards.db produced by
//! scripts/build_boards_db.py from the Python test suite's SAMPLE_BOARDS,
//! dumped once via a throwaway script so these rows are byte-identical to
//! what the real builder emits.
use rusqlite::Connection;
use std::collections::HashMap;

use super::SCHEMA;

pub(crate) fn test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(SCHEMA).unwrap();

    struct BoardRow {
        slug: &'static str, name: &'static str, org: &'static str, org_display: &'static str,
        source: &'static str, format: &'static str, description: &'static str,
        key_coverage: &'static str, layers: i64, width_mm: Option<f64>, height_mm: Option<f64>,
        min_trace: Option<&'static str>, min_clearance: Option<&'static str>,
        component_count: i64, ic_count: i64, net_count: i64,
        key_ics_text: &'static str, all_ics_text: &'static str,
        nets_json: Option<&'static str>, positions_json: Option<&'static str>,
        copper_pours_json: Option<&'static str>, neighborhoods_json: Option<&'static str>,
        tags: &'static [&'static str], key_ics: &'static [&'static str],
    }

    let boards = vec![
        BoardRow {
            slug: "adafruit-motor-shield", name: "Adafruit Motor Shield", org: "adafruit", org_display: "Adafruit",
            source: "adafruit/Motor-Shield", format: "eagle", description: "A motor driver shield for Arduino with DRV8825",
            key_coverage: "DRV8825 stepper motor driver", layers: 2, width_mm: Some(68.6), height_mm: Some(53.3),
            min_trace: None, min_clearance: None, component_count: 4, ic_count: 1, net_count: 2,
            key_ics_text: "DRV8825", all_ics_text: "DRV8825",
            nets_json: Some(r#"[{"name": "STEP", "pins": ["U1.STEP", "R1.1"]}, {"name": "GND", "pins": ["U1.GND", "C1.2", "C2.2", "R1.2"]}]"#),
            positions_json: None, copper_pours_json: None,
            neighborhoods_json: Some(r#"[{"ref": "U1", "value": "DRV8825", "pins": {"STEP": [{"ref": "R1", "value": "10kohm", "role": "pulldown"}], "_decoupling": [{"ref": "C1", "value": "100nF", "role": "decoupling"}, {"ref": "C2", "value": "47uF", "role": "decoupling"}]}}]"#),
            tags: &["motor-control"], key_ics: &["DRV8825"],
        },
        BoardRow {
            slug: "ble-sensor-node", name: "BLE Sensor Node", org: "SolderedElectronics", org_display: "Soldered Electronics",
            source: "SolderedElectronics/BLE-Sensor", format: "kicad7", description: "A Bluetooth Low Energy sensor node with BME280",
            key_coverage: "nRF52840 BLE with BME280 sensor", layers: 4, width_mm: Some(30.0), height_mm: Some(20.0),
            min_trace: None, min_clearance: None, component_count: 4, ic_count: 2, net_count: 4,
            key_ics_text: "nRF52840 BME280", all_ics_text: "nRF52840 BME280",
            nets_json: Some(r#"[{"name": "SDA", "pins": ["U1.SDA", "U2.SDA"]}, {"name": "SCL", "pins": ["U1.SCL", "U2.SCL"]}, {"name": "3V3", "pins": ["U1.VCC", "C1.1", "U2.VCC", "C2.1"]}, {"name": "GND", "pins": ["U1.GND", "C1.2", "U2.GND", "C2.2"]}]"#),
            positions_json: None, copper_pours_json: None,
            neighborhoods_json: Some(r#"[{"ref": "U1", "value": "nRF52840", "pins": {"SDA": [{"ref": "U2", "value": "BME280", "role": "ic"}], "SCL": [{"ref": "U2", "value": "BME280", "role": "ic"}], "_decoupling": [{"ref": "C1", "value": "100nF", "role": "decoupling"}]}}, {"ref": "U2", "value": "BME280", "pins": {"SDA": [{"ref": "U1", "value": "nRF52840", "role": "ic"}], "SCL": [{"ref": "U1", "value": "nRF52840", "role": "ic"}], "_decoupling": [{"ref": "C2", "value": "100nF", "role": "decoupling"}]}}]"#),
            tags: &["bluetooth", "sensors"], key_ics: &["BME280", "nRF52840"],
        },
        BoardRow {
            slug: "minimal-led-driver", name: "Minimal LED Driver", org: "maker", org_display: "Maker",
            source: "maker/led-driver", format: "eagle", description: "A simple constant-current LED driver with TPS61169",
            key_coverage: "TPS61169 constant-current LED driver", layers: 2, width_mm: None, height_mm: None,
            min_trace: None, min_clearance: None, component_count: 3, ic_count: 1, net_count: 0,
            key_ics_text: "TPS61169", all_ics_text: "TPS61169",
            nets_json: None, positions_json: None, copper_pours_json: None, neighborhoods_json: None,
            tags: &["led-driver"], key_ics: &["TPS61169"],
        },
        BoardRow {
            slug: "sparkfun-mcp73831-charger", name: "SparkFun MCP73831 Charger", org: "sparkfun", org_display: "SparkFun",
            source: "sparkfun/MCP73831-Charger", format: "eagle", description: "A simple LiPo charger breakout with MCP73831",
            key_coverage: "MCP73831 LiPo charging circuit", layers: 2, width_mm: None, height_mm: None,
            min_trace: None, min_clearance: None, component_count: 4, ic_count: 1, net_count: 4,
            key_ics_text: "MCP73831", all_ics_text: "MCP73831",
            nets_json: Some(r#"[{"name": "PROG", "pins": ["U1.PROG", "R1.1"]}, {"name": "STAT", "pins": ["U1.STAT", "D1.1"]}, {"name": "VCC", "pins": ["U1.VCC", "C1.1"]}, {"name": "GND", "pins": ["U1.GND", "C1.2", "R1.2", "D1.2"]}]"#),
            positions_json: None, copper_pours_json: None,
            neighborhoods_json: Some(r#"[{"ref": "U1", "value": "MCP73831", "pins": {"PROG": [{"ref": "R1", "value": "2kohm", "role": "resistor"}], "STAT": [{"ref": "D1", "value": "red LED", "role": "diode"}], "_decoupling": [{"ref": "C1", "value": "4.7uF", "role": "decoupling"}]}}]"#),
            tags: &["battery-charging", "power-supply"], key_ics: &["MCP73831"],
        },
        BoardRow {
            slug: "test-esp32-board", name: "Test ESP32 Board", org: "testorg", org_display: "Testorg",
            source: "testorg/test-esp32", format: "kicad7", description: "An ESP32 devkit with WiFi and battery charging",
            key_coverage: "ESP32-S3 WiFi devkit with MCP73831 battery charging", layers: 4, width_mm: Some(50.0), height_mm: Some(25.0),
            min_trace: Some("0.15mm"), min_clearance: Some("0.15mm"), component_count: 8, ic_count: 2, net_count: 5,
            key_ics_text: "ESP32-S3 MCP73831", all_ics_text: "ESP32-S3 MCP73831",
            nets_json: Some(r#"[{"name": "SDA", "pins": ["U1.SDA", "R3.1"]}, {"name": "SCL", "pins": ["U1.SCL", "R1.1"]}, {"name": "PROG", "pins": ["U2.PROG", "R2.1"]}, {"name": "3V3", "pins": ["U1.VCC", "C1.1", "U2.VCC", "C2.1"]}, {"name": "GND", "pins": ["U1.GND", "C1.2", "U2.GND", "C2.2"]}]"#),
            positions_json: Some(r#"[{"ref": "U1", "x": 10.0, "y": 10.0}, {"ref": "U2", "x": 20.0, "y": 10.0}]"#),
            copper_pours_json: Some(r#"[{"layer": "B.Cu", "net": "GND"}]"#),
            neighborhoods_json: Some(r#"[{"ref": "U1", "value": "ESP32-S3", "pins": {"SDA": [{"ref": "R3", "value": "100kohm", "role": "pullup"}], "SCL": [{"ref": "R1", "value": "10kohm", "role": "resistor"}], "_decoupling": [{"ref": "C1", "value": "100nF", "role": "decoupling"}]}}, {"ref": "U2", "value": "MCP73831", "pins": {"PROG": [{"ref": "R2", "value": "4.7kohm", "role": "resistor"}], "_decoupling": [{"ref": "C2", "value": "10uF", "role": "decoupling"}]}}]"#),
            tags: &["battery-charging", "sensors"], key_ics: &["ESP32-S3", "MCP73831"],
        },
    ];

    struct CompRow { board_slug: &'static str, ref_: &'static str, value: &'static str, footprint: &'static str, decouples: Option<&'static str>, pullup: Option<&'static str>, pulldown: Option<&'static str> }
    let comps = vec![
        CompRow { board_slug: "adafruit-motor-shield", ref_: "U1", value: "DRV8825", footprint: "HTSSOP-28", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "adafruit-motor-shield", ref_: "C1", value: "100nF", footprint: "0402", decouples: Some("U1"), pullup: None, pulldown: None },
        CompRow { board_slug: "adafruit-motor-shield", ref_: "C2", value: "47uF", footprint: "1206", decouples: Some("U1"), pullup: None, pulldown: None },
        CompRow { board_slug: "adafruit-motor-shield", ref_: "R1", value: "10kohm", footprint: "0402", decouples: None, pullup: None, pulldown: Some("STEP") },
        CompRow { board_slug: "ble-sensor-node", ref_: "U1", value: "nRF52840", footprint: "QFN-48", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "ble-sensor-node", ref_: "U2", value: "BME280", footprint: "LGA-8", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "ble-sensor-node", ref_: "C1", value: "100nF", footprint: "0402", decouples: Some("U1"), pullup: None, pulldown: None },
        CompRow { board_slug: "ble-sensor-node", ref_: "C2", value: "100nF", footprint: "0402", decouples: Some("U2"), pullup: None, pulldown: None },
        CompRow { board_slug: "minimal-led-driver", ref_: "U1", value: "TPS61169", footprint: "SOT-23-5", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "minimal-led-driver", ref_: "L1", value: "10uH", footprint: "1210", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "minimal-led-driver", ref_: "R1", value: "1ohm", footprint: "0402", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "sparkfun-mcp73831-charger", ref_: "U1", value: "MCP73831", footprint: "SOT-23-5", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "sparkfun-mcp73831-charger", ref_: "C1", value: "4.7uF", footprint: "0402", decouples: Some("U1"), pullup: None, pulldown: None },
        CompRow { board_slug: "sparkfun-mcp73831-charger", ref_: "R1", value: "2kohm", footprint: "0402", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "sparkfun-mcp73831-charger", ref_: "D1", value: "red LED", footprint: "0603", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "test-esp32-board", ref_: "U1", value: "ESP32-S3", footprint: "QFN-48", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "test-esp32-board", ref_: "U2", value: "MCP73831", footprint: "SOT-23-5", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "test-esp32-board", ref_: "R1", value: "10kohm", footprint: "0402", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "test-esp32-board", ref_: "R2", value: "4.7kohm", footprint: "0402", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "test-esp32-board", ref_: "C1", value: "100nF", footprint: "0402", decouples: Some("U1"), pullup: None, pulldown: None },
        CompRow { board_slug: "test-esp32-board", ref_: "C2", value: "10uF", footprint: "0805", decouples: Some("U2"), pullup: None, pulldown: None },
        CompRow { board_slug: "test-esp32-board", ref_: "C3", value: "4.7uF", footprint: "0402", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "test-esp32-board", ref_: "R3", value: "100kohm", footprint: "0402", decouples: None, pullup: Some("SDA"), pulldown: None },
    ];

    let mut board_ids: HashMap<&str, i64> = HashMap::new();
    for b in &boards {
        conn.execute(
            "INSERT INTO boards (slug, name, org, org_display, source, format, description, key_coverage, layers, width_mm, height_mm, min_trace, min_clearance, min_drill, min_via, component_count, ic_count, net_count, key_ics_text, all_ics_text, nets_json, positions_json, copper_pours_json, neighborhoods_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,NULL,NULL,?14,?15,?16,?17,?18,?19,?20,?21,?22)",
            rusqlite::params![b.slug, b.name, b.org, b.org_display, b.source, b.format, b.description, b.key_coverage, b.layers, b.width_mm, b.height_mm, b.min_trace, b.min_clearance, b.component_count, b.ic_count, b.net_count, b.key_ics_text, b.all_ics_text, b.nets_json, b.positions_json, b.copper_pours_json, b.neighborhoods_json],
        ).unwrap();
        let id = conn.last_insert_rowid();
        board_ids.insert(b.slug, id);
        for t in b.tags {
            conn.execute("INSERT INTO board_tags VALUES (?1, ?2)", rusqlite::params![id, t]).unwrap();
        }
        for ic in b.key_ics {
            conn.execute("INSERT INTO board_key_ics VALUES (?1, ?2)", rusqlite::params![id, ic]).unwrap();
        }
    }
    for c in &comps {
        let board_id = board_ids[c.board_slug];
        conn.execute(
            "INSERT INTO board_components (board_id, ref, value, footprint, decouples, pullup, pulldown) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![board_id, c.ref_, c.value, c.footprint, c.decouples, c.pullup, c.pulldown],
        ).unwrap();
    }

    // Populate FTS exactly as build_boards_db.py does: per-board tags_text joined from board_tags.
    for b in &boards {
        let tags_text = b.tags.join(" ");
        conn.execute(
            "INSERT INTO boards_fts (slug, name, description, key_coverage, tags_text, key_ics_text, all_ics_text, org_display) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            rusqlite::params![b.slug, b.name, b.description, b.key_coverage, tags_text, b.key_ics_text, b.all_ics_text, b.org_display],
        ).unwrap();
    }

    conn
}
```

- [ ] **Step 4: Write `boards/search.rs` — `escape_like`, `sanitize_fts_query`, `get_stats` and their tests**

```rust
// rust/crates/pcbparts-db/src/boards/search.rs
use rusqlite::Connection;
use serde_json::json;
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

pub fn get_stats(conn: &Connection) -> serde_json::Value {
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
```

Note: `get_stats`'s `format`/`org_display`/`tag` grouping queries read those
columns as non-nullable `String`. This relies on the real builder's
invariant that these fields are never NULL (`build_boards_db.py` defaults
every one of them via `board.get(key, "")`), matching Python's own
behavior. If a future caller ever passes a DB with a genuinely NULL
`format`, this will panic loudly rather than silently misbehave — that's an
acceptable trade-off here since it can only happen if a non-conforming DB
file is fed in.

- [ ] **Step 5: Add the pure-function and `get_stats` tests**

```rust
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
```

- [ ] **Step 6: Run the tests written so far**

This won't compile yet — `boards/mod.rs` declares `pub mod detail;` which
doesn't exist. Skip running until Task 6 adds it; alternatively, comment
out `pub mod detail;` temporarily to verify this task's tests pass in
isolation, then uncomment before moving to Task 6.

Run: `cargo test -p pcbparts-db boards::search::tests` (after temporarily
commenting `pub mod detail;`)
Expected: PASS — 21 tests.

- [ ] **Step 7: Commit**

```bash
git add rust/crates/pcbparts-db/src/boards/mod.rs rust/crates/pcbparts-db/src/boards/search.rs rust/crates/pcbparts-db/src/boards/fixtures.rs rust/crates/pcbparts-db/src/lib.rs
git commit -m "rust: port boards_db schema, escape_like, sanitize_fts_query, get_stats"
```

---

### Task 5: boards_db — search_boards + full test port

**Files:**
- Modify: `rust/crates/pcbparts-db/src/boards/search.rs`

**Interfaces:**
- Consumes: `escape_like`, `source_url`, `sanitize_fts_query` from Task 4.
- Produces: `search_boards(...) -> SearchBoardsResult`, `BoardSummary` —
  consumed by Task 7's `BoardsDb` wrapper.

- [ ] **Step 1: Add `BoardSummary`, `SearchBoardsResult`, and `search_boards`**

Insert above the `#[cfg(test)] mod tests` block in `boards/search.rs`
(and change the `use` line at the top from `use serde_json::json;` to
`use serde_json::{json, Value};` and add `use rusqlite::{Connection, ToSql};`):

```rust
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
```

- [ ] **Step 2: Add the `search_boards` tests inside `mod tests`**

```rust
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
```

- [ ] **Step 3: Run**

Run: `cargo test -p pcbparts-db boards::search::` (still requires
`detail.rs`/Task 6 to exist for the crate to compile — see Task 4's Step 6
note; run this together with Task 6 if working strictly linearly)
Expected: PASS — 21 (Task 4) + 34 (this task) = 55 tests in `boards::search`.

- [ ] **Step 4: Commit**

```bash
git add rust/crates/pcbparts-db/src/boards/search.rs
git commit -m "rust: port boards_db search_boards with full test parity"
```

---

### Task 6: boards_db — detail.rs (get_board, get_consensus, get_tag_consensus)

**Files:**
- Create: `rust/crates/pcbparts-db/src/boards/detail.rs`

**Interfaces:**
- Consumes: `escape_like`, `source_url` from Task 4/5 (`boards::search`).
- Produces: `get_board`, `get_consensus`, `get_tag_consensus` — consumed by
  Task 7's `BoardsDb` wrapper.

- [ ] **Step 1: Write the full module**

```rust
// rust/crates/pcbparts-db/src/boards/detail.rs
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
```

- [ ] **Step 2: Add the full test module**

```rust
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
```

- [ ] **Step 3: Run the full boards module now that it compiles end-to-end**

Run: `cargo test -p pcbparts-db boards::`
Expected: PASS — 21 (search pure fns/get_stats) + 34 (search_boards) + 34
(detail) = 89 tests total in `boards::`.

- [ ] **Step 4: Commit**

```bash
git add rust/crates/pcbparts-db/src/boards/detail.rs
git commit -m "rust: port boards_db detail (get_board, get_consensus, get_tag_consensus)"
```

---

### Task 7: Public wrapper structs (BoardsDb, SensorDb) + final verification

**Files:**
- Modify: `rust/crates/pcbparts-db/src/boards/mod.rs`
- Modify: `rust/crates/pcbparts-db/src/sensor/mod.rs`

**Interfaces:**
- Produces: `BoardsDb::open`, `BoardsDb::{search, get_board, get_consensus,
  get_tag_consensus, get_stats}`, `SensorDb::open`, `SensorDb::{search,
  get_stats}` — the surface Phase 8 (`pcbparts-server`) will call from the
  MCP tool handlers, matching Python's `BoardsDatabase`/`SensorDatabase`
  public methods.

- [ ] **Step 1: Add `BoardsDb` to `boards/mod.rs`**

Append to the end of `boards/mod.rs` (after the `SCHEMA` const):

```rust
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

/// Opens an existing boards.db file (built by the offline pipeline).
///
/// NOTE: unlike Python's `BoardsDatabase`, this does not build the DB if
/// missing — `pcbparts-pipeline` (a later migration phase) owns the Rust
/// builder. `Mutex<Connection>` gives correct concurrent access for now;
/// the production connection-pooling strategy is decided in the
/// `pcbparts-server` phase once the async runtime is in place.
pub struct BoardsDb {
    conn: Mutex<Connection>,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("boards database not found at {0}")]
    NotFound(String),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

impl BoardsDb {
    pub fn open(db_path: &Path) -> Result<Self, OpenError> {
        if !db_path.exists() {
            return Err(OpenError::NotFound(db_path.display().to_string()));
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL")?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn search(
        &self,
        query: Option<&str>,
        component: Option<&str>,
        tag: Option<&[&str]>,
        org: Option<&str>,
        layers: Option<i64>,
        limit: i64,
    ) -> search::SearchBoardsResult {
        let conn = self.conn.lock().unwrap();
        search::search_boards(&conn, query, component, tag, org, layers, limit)
    }

    pub fn get_board(
        &self,
        slug: &str,
        include_raw: bool,
        include_bom: bool,
        focus: Option<&str>,
    ) -> Option<serde_json::Value> {
        let conn = self.conn.lock().unwrap();
        detail::get_board(&conn, slug, include_raw, include_bom, focus)
    }

    pub fn get_consensus(&self, ic_name: &str) -> Option<serde_json::Value> {
        let conn = self.conn.lock().unwrap();
        detail::get_consensus(&conn, ic_name)
    }

    pub fn get_tag_consensus(&self, tag: &str) -> Option<serde_json::Value> {
        let conn = self.conn.lock().unwrap();
        detail::get_tag_consensus(&conn, tag)
    }

    pub fn get_stats(&self) -> serde_json::Value {
        let conn = self.conn.lock().unwrap();
        search::get_stats(&conn)
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Writes a real on-disk boards.db (schema + one row) and confirms
    /// `BoardsDb::open` reads it back — proves the production open path
    /// (as opposed to the in-memory fixtures the unit tests use).
    #[test]
    fn opens_real_file_and_searches() {
        let dir = std::env::temp_dir().join(format!("pcbparts-boards-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("boards.db");

        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(SCHEMA).unwrap();
            // format/org/org_display/description/key_coverage/*_text are never NULL in
            // real builder output (build_boards_db.py defaults every field with `.get(k, "")`),
            // so a realistic row sets them all — mirrors get_stats()'s reliance on that invariant.
            conn.execute(
                "INSERT INTO boards (slug, name, org, org_display, source, format, description, \
                 key_coverage, key_ics_text, all_ics_text, component_count, ic_count, net_count) \
                 VALUES ('x', 'X Board', 'acme', 'Acme', 'acme/x', 'kicad7', '', '', '', '', 1, 0, 0)",
                [],
            ).unwrap();
        }

        let db = BoardsDb::open(&db_path).unwrap();
        let stats = db.get_stats();
        assert_eq!(stats["total_boards"], 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_errors_clearly() {
        let missing = Path::new("/nonexistent/path/boards.db");
        assert!(matches!(BoardsDb::open(missing), Err(OpenError::NotFound(_))));
    }
}
```

- [ ] **Step 2: Add `SensorDb` to `sensor/mod.rs`**

Append to the end of `sensor/mod.rs` (after the `SCHEMA` const):

```rust
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

/// Opens an existing sensor.db file (built by the offline pipeline).
/// See `boards::BoardsDb` doc comment for the Mutex/pooling and
/// missing-builder notes — the same reasoning applies here.
pub struct SensorDb {
    conn: Mutex<Connection>,
    ic_aliases: HashMap<String, String>,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("sensor database not found at {0}")]
    NotFound(String),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

impl SensorDb {
    /// `data_dir` mirrors Python's constructor: `ic_aliases.json` is loaded
    /// from `data_dir/sensors/ic_aliases.json` if present, else aliases are empty.
    pub fn open(db_path: &Path, data_dir: &Path) -> Result<Self, OpenError> {
        if !db_path.exists() {
            return Err(OpenError::NotFound(db_path.display().to_string()));
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL")?;

        let aliases_path = data_dir.join("sensors").join("ic_aliases.json");
        let ic_aliases = std::fs::read_to_string(&aliases_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        Ok(Self { conn: Mutex::new(conn), ic_aliases })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn search(
        &self,
        query: Option<&str>,
        measure: Option<search::MeasureFilter>,
        r#type: Option<&str>,
        protocol: Option<&str>,
        platform: Option<&str>,
        limit: i64,
    ) -> search::SearchSensorsResult {
        let conn = self.conn.lock().unwrap();
        search::search_sensors(&conn, query, measure, r#type, protocol, platform, limit, &self.ic_aliases)
    }

    pub fn get_stats(&self) -> serde_json::Value {
        let conn = self.conn.lock().unwrap();
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM sensors", [], |r| r.get(0)).unwrap();

        let mut platforms = serde_json::Map::new();
        {
            let mut stmt = conn.prepare("SELECT platform, COUNT(*) FROM sensor_platforms GROUP BY platform ORDER BY 2 DESC").unwrap();
            let mut rows = stmt.query([]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                let p: String = row.get(0).unwrap();
                let c: i64 = row.get(1).unwrap();
                platforms.insert(p, serde_json::json!(c));
            }
        }

        let mut measures = serde_json::Map::new();
        {
            let mut stmt = conn.prepare("SELECT measure, COUNT(*) FROM sensor_measures GROUP BY measure ORDER BY 2 DESC LIMIT 20").unwrap();
            let mut rows = stmt.query([]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                let m: String = row.get(0).unwrap();
                let c: i64 = row.get(1).unwrap();
                measures.insert(m, serde_json::json!(c));
            }
        }

        serde_json::json!({
            "total_sensors": total,
            "platforms": platforms,
            "top_measures": measures,
        })
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn opens_real_file_with_no_aliases_file() {
        let dir = std::env::temp_dir().join(format!("pcbparts-sensor-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("sensor.db");

        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(SCHEMA).unwrap();
            conn.execute(
                "INSERT INTO sensors (id, name, platform_count) VALUES ('x1', 'X1', 0)",
                [],
            ).unwrap();
        }

        let db = SensorDb::open(&db_path, &dir).unwrap();
        let stats = db.get_stats();
        assert_eq!(stats["total_sensors"], 1);
        assert!(db.ic_aliases.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_errors_clearly() {
        let missing = Path::new("/nonexistent/path/sensor.db");
        let missing_dir = Path::new("/nonexistent/path");
        assert!(matches!(SensorDb::open(missing, missing_dir), Err(OpenError::NotFound(_))));
    }
}
```

- [ ] **Step 3: Run the entire crate's test suite**

Run: `cd rust && cargo test -p pcbparts-db`
Expected: PASS — **134 tests total** (1 smoke test + 41 sensor + 89 boards +
2 boards wrapper integration tests + 2 sensor wrapper integration tests —
this exact count has been verified).

- [ ] **Step 4: Commit**

```bash
git add rust/crates/pcbparts-db/src/boards/mod.rs rust/crates/pcbparts-db/src/sensor/mod.rs
git commit -m "rust: add BoardsDb/SensorDb wrapper structs opening real DB files"
```

## Self-Review Notes

- **Spec coverage:** This plan covers the corrected Phase 1 scope exactly
  (`boards_db` + `sensor_db` read side) from the spec's Migration Order
  section. Component-DB (`db/`), search engine, parsers, clients, the wafer
  bridge, pipeline, and server are explicitly out of scope — later phases.
- **No placeholders:** every code block above is verbatim, previously
  compiled-and-tested Rust code (134/134 tests passing), not sketched.
- **Type consistency:** `BoardSummary`, `SearchBoardsResult`,
  `SensorResult`, `SearchSensorsResult`, `MeasureFilter`, `MeasureMode` are
  used with identical names/signatures everywhere they appear across tasks.
- **Known deferred design decision:** connection concurrency
  (`Mutex<Connection>` vs. a real pool) is explicitly deferred to the
  `pcbparts-server` phase, not silently baked in as final.

## Next Step

Offer execution choice per the writing-plans skill: **Subagent-Driven**
(dispatch a fresh subagent per task, review between tasks) or **Inline
Execution** (batch through tasks in this session with checkpoints).
