# Rust Migration Phase 4: pcbparts-smart-parser Crate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port `src/pcbparts_mcp/smart_parser/*.py` (the free-text query parser that turns
"10k resistor 0603 1%" or "100V mosfet" into structured search filters) into a new
`pcbparts-smart-parser` Rust crate, with every existing pytest assertion translated 1:1
into a passing Rust test and every zero-coverage function backed by a characterization
test captured from the live Python implementation.

**Architecture:** One Rust module per Python file, the same convention Phase 2A and
Phase 3 established — `values.rs`, `models.rs`, `packages.rs`, `connectors.rs`,
`semantic.rs`, `types.rs`, `mapping.rs`, `parser.rs`. Tasks are ordered by the crate's
real intra-module dependency graph, verified from the actual `from .X import Y`
statements in each Python file (not assumed):

- `values.rs`, `models.rs`, `packages.rs`, `connectors.rs`, `semantic.rs` have **no
  intra-package imports** — each is stdlib-only Python (`re`, `dataclasses`) with zero
  dependency on any other `smart_parser` file.
- `types.rs` imports `from ..subcategory_aliases import SUBCATEGORY_ALIASES` — a
  cross-crate dependency on Phase 2A's `pcbparts-parsers::subcategory_aliases`.
- `mapping.rs` imports `from .values import ExtractedValue` — an intra-crate dependency
  on `values.rs`, so `values.rs` must exist first.
- `parser.rs` imports `from ..search.spec_filter import SpecFilter` (a cross-crate
  dependency on Phase 3's `pcbparts-search::spec_filter::SpecFilter`) plus
  `from .packages import extract_package`, `from .values import ExtractedValue,
  extract_values`, `from .models import extract_model_number`, `from .types import
  extract_component_type, extract_mounting_type`, `from .semantic import
  extract_semantic_descriptors, remove_noise_words, CONNECTOR_NOISE_WORDS`, `from
  .mapping import map_value_to_spec, infer_subcategory_from_values`, `from .connectors
  import extract_connector_series, ConnectorSpec` — every other module in this crate.
  It is ported last.

Two of this crate's regexes use lookaround assertions (`(?<=\s)` in `values.py`,
`(?!\s*connector)` in `packages.py`) that Rust's `regex` crate does not support (it uses
a linear-time automaton, not a backtracking engine, and lookaround requires
backtracking). Both are worked around with a documented post-hoc filter over an
unconstrained match — see Task 1 and Task 2 below. This is the first place in the
migration that hits this limitation; no file ported in Phase 1, 2A, 2B, or 3 used
lookaround.

**Tech Stack:** Rust 2021 edition, `regex` (matching every prior phase's version),
`serde_json` with the `preserve_order` feature (for `ParsedQuery::detected`, a
dynamic dict-shaped debug/introspection field — same "dynamic dict-shaped values"
treatment Phase 2A gave `pinout.rs`/`design_rules.rs`), `pcbparts-parsers` and
`pcbparts-search` as path dependencies.

**Spec:** `docs/superpowers/specs/2026-08-22-rust-migration-design.md`

## Global Constraints

- **Golden-value parity, two flavors, verified per-file.** `models.rs`, `packages.rs`,
  `connectors.rs`, and the `remove_noise_words` half of `semantic.rs` have real,
  pre-existing pytest coverage in `tests/test_parsers.py` — ported 1:1, exact assertions.
  `values.rs`, `types.rs`, `mapping.rs`, the `extract_semantic_descriptors` half of
  `semantic.rs`, and `merge_spec_filters` in `parser.rs` have **zero existing pytest
  coverage** (confirmed: none of `values`, `types`, `mapping` is imported anywhere in
  `tests/test_parsers.py`, and `extract_semantic_descriptors`/`merge_spec_filters` are
  never called directly by any test — only indirectly through `parse_smart_query`, which
  itself is only exercised by 8 of the many code paths it contains). These get
  characterization tests: the actual Python functions were run against representative
  inputs in this repo's own `.venv` during this plan's authoring, and their real output
  is embedded as literal Rust assertions in each task below — not hand-derived, not
  guessed. If a task's test fails against an embedded fixture, the port has a bug, not
  the fixture.
- **Two lookaround-regex deviations, both required and both documented in place.**
  (1) `values.py`'s `_CURR = re.compile(r'(?:^|(?<=\s))(\d+(?:\.\d+)?)\s*([u]?[mM]?)[aA]\b')`
  uses a lookbehind to avoid matching current values embedded in model-number suffixes
  (e.g. the "6.0A" inside "SMBJ6.0A"). Ported as an unconstrained pattern
  `(\d+(?:\.\d+)?)\s*([u]?[mM]?)[aA]\b` plus a `preceded_by_start_or_space` post-filter
  applied to each candidate match — behaviorally identical to the lookbehind because the
  lookbehind is zero-width and does not shift match boundaries, so filtering candidates
  post-hoc reproduces exactly what `re.finditer` with the lookbehind would have returned.
  (2) `packages.py`'s SMA/SMB/SMC pattern
  `r'\b(SM[ABC])\b(?!\s*connector)'` uses a negative lookahead to avoid treating "SMA" in
  "SMA connector" as a package name. Ported as an unconstrained `\b(SM[ABC])\b` pattern
  plus a `find_diode_pkg` helper that iterates all candidate matches left-to-right and
  returns the first one NOT followed by `connector` (optionally preceded by whitespace)
  — this reproduces `re.search`'s "leftmost position satisfying the whole pattern,
  backtracking past positions that fail the lookahead" semantics exactly, since there is
  no other alternation at play that could make the two approaches diverge.
- **`subcategory` (Step 3's local binding) vs. `result.subcategory` (the mutable field)
  are NOT interchangeable and Python's `parser.py` deliberately reads from different ones
  at different points — this is the single easiest thing to get wrong porting this file.**
  After Step 3 unpacks `subcategory, remaining, matched_keyword =
  extract_component_type(remaining)`, `result.subcategory` may be reassigned later (Step
  4b's inference, Step 4c's trimmer/potentiometer override) while the local `subcategory`
  variable keeps its Step-3 value forever. Step 6's `map_value_to_spec(value,
  subcategory, matched_keyword)` call and Step 7a/7b's connector/header text-cleanup
  checks read the **local** `subcategory`; Step 6's `subcat_lower` (dimension-as-package
  routing) and Step 6b's "dual" MOSFET check read **`result.subcategory`**. Task 7 below
  preserves this distinction explicitly — do not "simplify" by collapsing them to one
  variable.
- **`ParsedQuery.detected` is a debug/introspection field with zero pytest assertions
  checking its contents in any test** (confirmed: no test in `test_parsers.py` or
  `test_db.py` inspects `result.detected`). It is ported as a `serde_json::Value` for API
  parity — populated the same way at the same points Python populates its `detected`
  dict — but is **not** exhaustively fixture-verified key-by-key the way `spec_filters`,
  `subcategory`, `package`, etc. are, since there is no existing behavior contract to
  break. This is a deliberate, bounded scope reduction, not an oversight.
- **`tests/test_db.py`'s smart_parser-integration tests are explicitly OUT of scope for
  this phase.** `grep -rn smart_parser tests/` finds exactly two files:
  `tests/test_parsers.py` (in scope, ported below) and `tests/test_db.py` (15 call sites,
  all `from pcbparts_mcp.smart_parser import parse_smart_query` followed by running the
  result through a live `ComponentDatabase.search()` call against a real SQLite
  database). Those tests exercise `parse_smart_query` **plus** Phase 5's component-DB
  search layer together — per the spec's migration-order note, "port what's portable
  here, finish the rest in phase 5." Nothing in this plan attempts them; Phase 5's plan
  is responsible for the DB-integration half once `pcbparts-db`'s component half exists.
- **No `pcbparts-db` dependency.** `smart_parser/*.py` never touches SQLite directly —
  it only builds `ParsedQuery` values that a caller (Phase 5's search layer, eventually)
  feeds into `SearchEngine::search()`. `pcbparts-search`'s own `Cargo.toml` doesn't
  declare `pcbparts-db` either (confirmed by reading it) — this crate follows the same
  precedent and depends only on `pcbparts-parsers` and `pcbparts-search`.
  `DEFAULT_MIN_STOCK`/`config.py` are not relevant to this crate at all — `smart_parser`
  never imports `config.py`.
- Every ported test must assert the same behavior as its Python counterpart or the
  captured characterization fixture (golden-value parity), not a re-derived expectation.
- Per CLAUDE.md and the `project-rust-rewrite` memory: never commit without explicit
  permission, no Claude attribution in commit messages.

## File Structure

```
rust/crates/pcbparts-smart-parser/
  Cargo.toml
  src/
    lib.rs           # pub mod declarations + pub use re-exports (mirrors __init__.py)
    values.rs         # ExtractedValue, extract_values
    models.rs          # extract_model_number
    packages.rs         # extract_package
    connectors.rs        # ConnectorSpec, extract_connector_series, get_pitch_for_series
    semantic.rs           # SemanticFilter, extract_semantic_descriptors, remove_noise_words
    types.rs               # extract_component_type, extract_mounting_type
    mapping.rs               # map_value_to_spec, infer_subcategory_from_values
    parser.rs                 # ParsedQuery, parse_smart_query, merge_spec_filters
```

---

### Task 1: Crate scaffold + `values.rs`

**Files:**
- Create: `rust/crates/pcbparts-smart-parser/Cargo.toml`
- Create: `rust/crates/pcbparts-smart-parser/src/lib.rs`
- Create: `rust/crates/pcbparts-smart-parser/src/values.rs`
- Modify: `rust/Cargo.toml` (add the new crate to `members`)

**Interfaces:**
- Consumes: nothing from earlier phases (stdlib-only, matching `values.py`).
- Produces: `ExtractedValue { raw: String, value: f64, unit_type: String, normalized:
  String }` and `extract_values(query: &str) -> (Vec<ExtractedValue>, String)` —
  consumed by Task 6's `mapping.rs` (`ExtractedValue` as a parameter type) and Task 7's
  `parser.rs` (`extract_values` called directly, `ExtractedValue` constructed for the
  Step 4a/4d standalone-number cases).

- [ ] **Step 1: Add the crate to the workspace**

```toml
# rust/Cargo.toml
[workspace]
resolver = "2"
members = [
    "crates/pcbparts-db",
    "crates/pcbparts-parsers",
    "crates/pcbparts-search",
    "crates/pcbparts-smart-parser",
]
```

- [ ] **Step 2: Create the crate manifest**

Every cross-crate and cross-file dependency this crate will ever need (`types.rs` in
Task 5 needs `pcbparts-parsers`; `parser.rs` in Task 7 needs `pcbparts-search`) is
declared up front here, matching the precedent Phase 3's Task 1 set (its Cargo.toml
declared `rusqlite` and `pcbparts-db` even though `mpn.rs` itself used neither).

```toml
# rust/crates/pcbparts-smart-parser/Cargo.toml
[package]
name = "pcbparts-smart-parser"
version = "0.1.0"
edition = "2021"

[dependencies]
serde_json = { version = "1", features = ["preserve_order"] }
regex = "1"
pcbparts-parsers = { path = "../pcbparts-parsers" }
pcbparts-search = { path = "../pcbparts-search" }
```

- [ ] **Step 3: Write `lib.rs`**

Only `values` for now — each later task adds its own `pub mod X;` line when it creates
that module, the same incremental-declaration discipline Phase 2A and Phase 3 both used.

```rust
pub mod values;
```

- [ ] **Step 4: Write the failing tests for `values.rs`**

`values.py` has zero existing pytest coverage. The fixtures below are real output,
captured by running `.venv/bin/python3 -c "from pcbparts_mcp.smart_parser.values import
extract_values; ..."` against this repo's own installed package during this plan's
authoring.

```rust
// rust/crates/pcbparts-smart-parser/src/values.rs — tests module (write this first)
#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} != {b}");
    }

    #[test]
    fn resistance_and_tolerance() {
        let (values, remaining) = extract_values("10k resistor 0603 1%");
        assert_eq!(remaining, "resistor 0603");
        assert_eq!(values.len(), 2);
        assert_eq!(values[0].raw, "10k");
        approx(values[0].value, 10000.0);
        assert_eq!(values[0].unit_type, "resistance");
        assert_eq!(values[0].normalized, "10kOhm");
        assert_eq!(values[1].raw, "1%");
        approx(values[1].value, 1.0);
        assert_eq!(values[1].unit_type, "tolerance");
        assert_eq!(values[1].normalized, "1%");
    }

    #[test]
    fn capacitance_and_voltage() {
        let (values, remaining) = extract_values("100nF 50V");
        assert_eq!(remaining, "");
        assert_eq!(values.len(), 2);
        approx(values[0].value, 100e-9);
        assert_eq!(values[0].unit_type, "capacitance");
        assert_eq!(values[0].normalized, "100nF");
        approx(values[1].value, 50.0);
        assert_eq!(values[1].unit_type, "voltage");
        assert_eq!(values[1].normalized, "50V");
    }

    #[test]
    fn inductance_and_current() {
        let (values, remaining) = extract_values("4.7uH 2A");
        assert_eq!(remaining, "");
        assert_eq!(values.len(), 2);
        approx(values[0].value, 4.7e-6);
        assert_eq!(values[0].unit_type, "inductance");
        assert_eq!(values[0].normalized, "4.7uH");
        approx(values[1].value, 2.0);
        assert_eq!(values[1].unit_type, "current");
        assert_eq!(values[1].normalized, "2A");
    }

    #[test]
    fn frequency_leaves_remaining_text() {
        let (values, remaining) = extract_values("8MHz crystal");
        assert_eq!(remaining, "crystal");
        assert_eq!(values.len(), 1);
        approx(values[0].value, 8_000_000.0);
        assert_eq!(values[0].unit_type, "frequency");
        assert_eq!(values[0].normalized, "8MHz");
    }

    #[test]
    fn pin_count() {
        let (values, remaining) = extract_values("16 pin header");
        assert_eq!(remaining, "header");
        assert_eq!(values.len(), 1);
        approx(values[0].value, 16.0);
        assert_eq!(values[0].unit_type, "pin_count");
        assert_eq!(values[0].normalized, "16P");
    }

    #[test]
    fn pin_structure() {
        let (values, remaining) = extract_values("2x20 header");
        assert_eq!(remaining, "header");
        assert_eq!(values.len(), 1);
        approx(values[0].value, 40.0);
        assert_eq!(values[0].unit_type, "pin_structure");
        assert_eq!(values[0].normalized, "2x20P");
    }

    #[test]
    fn pitch_and_pin_count_together() {
        let (values, remaining) = extract_values("2.54mm pitch header 8 pin");
        assert_eq!(remaining, "header");
        assert_eq!(values.len(), 2);
        approx(values[0].value, 2.54);
        assert_eq!(values[0].unit_type, "pitch");
        assert_eq!(values[0].normalized, "2.54mm");
        approx(values[1].value, 8.0);
        assert_eq!(values[1].unit_type, "pin_count");
        assert_eq!(values[1].normalized, "8P");
    }

    #[test]
    fn power_fraction_and_resistance() {
        let (values, remaining) = extract_values("1/4W 100 ohm resistor");
        assert_eq!(remaining, "resistor");
        assert_eq!(values.len(), 2);
        approx(values[0].value, 0.25);
        assert_eq!(values[0].unit_type, "power");
        assert_eq!(values[0].normalized, "1/4W");
        approx(values[1].value, 100.0);
        assert_eq!(values[1].unit_type, "resistance");
        assert_eq!(values[1].normalized, "100Ohm");
    }

    #[test]
    fn dimensions_under_100_kept() {
        let (values, remaining) = extract_values("6x6mm push button");
        assert_eq!(remaining, "push button");
        assert_eq!(values.len(), 1);
        approx(values[0].value, 6006.0); // encoded as x*1000 + y
        assert_eq!(values[0].unit_type, "dimensions");
        assert_eq!(values[0].normalized, "6x6mm");
    }

    #[test]
    fn voltage_and_current_no_remaining() {
        let (values, remaining) = extract_values("3.3V 500mA");
        assert_eq!(remaining, "");
        assert_eq!(values.len(), 2);
        approx(values[0].value, 3.3);
        assert_eq!(values[0].unit_type, "voltage");
        approx(values[1].value, 0.5);
        assert_eq!(values[1].unit_type, "current");
        assert_eq!(values[1].normalized, "500mA");
    }

    #[test]
    fn capacitance_voltage_tolerance_three_way() {
        let (values, remaining) = extract_values("10uF 25V 20%");
        assert_eq!(remaining, "");
        assert_eq!(values.len(), 3);
        approx(values[0].value, 10e-6);
        assert_eq!(values[0].unit_type, "capacitance");
        approx(values[1].value, 25.0);
        assert_eq!(values[1].unit_type, "voltage");
        approx(values[2].value, 20.0);
        assert_eq!(values[2].unit_type, "tolerance");
    }

    #[test]
    fn current_not_matched_inside_model_suffix() {
        // Regression case for the lookbehind workaround: "6.0A" inside "SMBJ6.0A" is
        // NOT preceded by start-of-string or whitespace, so it must not be extracted
        // as a current value (matching Python's `(?:^|(?<=\s))` lookbehind).
        let (values, _remaining) = extract_values("SMBJ6.0A TVS diode");
        assert!(values.iter().all(|v| v.unit_type != "current"));
    }

    #[test]
    fn current_matched_at_start_of_string() {
        let (values, remaining) = extract_values("2A fuse");
        assert_eq!(remaining, "fuse");
        assert_eq!(values.len(), 1);
        approx(values[0].value, 2.0);
        assert_eq!(values[0].unit_type, "current");
    }
}
```

- [ ] **Step 5: Run the tests to verify they fail**

Run: `cd rust && cargo test -p pcbparts-smart-parser`
Expected: FAIL to compile — `ExtractedValue`/`extract_values` don't exist yet.

- [ ] **Step 6: Write the implementation**

```rust
// rust/crates/pcbparts-smart-parser/src/values.rs — insert above the tests module
use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedValue {
    pub raw: String,
    pub value: f64,
    pub unit_type: String,
    pub normalized: String,
}

impl ExtractedValue {
    fn new(raw: impl Into<String>, value: f64, unit_type: &str, normalized: impl Into<String>) -> Self {
        Self { raw: raw.into(), value, unit_type: unit_type.to_string(), normalized: normalized.into() }
    }
}

static RES_EURO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(\d+)([kKmMrR])(\d+)\b").unwrap());
static RES_STD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(\d+(?:\.\d+)?)\s*([kKmMrROhm]|ohm|kohm|mohm)\b").unwrap());
static CAP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d+(?:\.\d+)?)\s*(u[fF]|n[fF]|p[fF]|[u]F|nF|pF)\b").unwrap());
static IND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d+(?:\.\d+)?)\s*(u[hH]|n[hH]|m[hH]|[u]H|nH|mH)\b").unwrap());
static VOLT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(\d+(?:\.\d+)?)\s*([kK])?[vV]\b").unwrap());
// Python's `_CURR` pattern opens with a lookbehind `(?:^|(?<=\s))` that the `regex`
// crate cannot express. Matched unconstrained here; `preceded_by_start_or_space`
// below reproduces the lookbehind by filtering candidate matches post-hoc — this is
// exact, not approximate, because the lookbehind is zero-width and never shifts a
// match's start/end offsets.
static CURR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+(?:\.\d+)?)\s*([u]?[mM]?)[aA]\b").unwrap());
static FREQ: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(\d+(?:\.\d+)?)\s*([kKmMgG])?[hH][zZ]\b").unwrap());
static TOL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(\d+(?:\.\d+)?)\s*%").unwrap());
static POWER_FRAC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(\d+)/(\d+)\s*[wW]\b").unwrap());
static POWER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(\d+(?:\.\d+)?)\s*([mM])?[wW]\b").unwrap());
static PINS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(\d+)\s*-?pins?\b").unwrap());
static DIM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d+(?:\.\d+)?)\s*[xX]\s*(\d+(?:\.\d+)?)\s*(?:mm)?\b").unwrap());
static PITCH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(\d+(?:\.\d+)?)\s*mm(?:\s+pitch)?\b").unwrap());
static POSITION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(\d+)\s*-?\s*(?:pos(?:ition)?|way|P)\b").unwrap());
static PIN_STRUCTURE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b([12])\s*[xX]\s*(\d+)\b").unwrap());
static WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

// Python's values.py also defines `_TEMP = re.compile(r'\b([+-]?\d+)\s*[C]?C\b',
// re.IGNORECASE)` at module scope but never references it inside `extract_values` (or
// anywhere else in the module) — confirmed dead code by re-reading the full source.
// Intentionally not ported: there is no observable behavior tied to it.

fn preceded_by_start_or_space(query: &str, byte_pos: usize) -> bool {
    byte_pos == 0 || query[..byte_pos].chars().next_back().is_some_and(|c| c.is_whitespace())
}

fn preceded_by_letter(query: &str, byte_pos: usize) -> bool {
    byte_pos > 0 && query[..byte_pos].chars().next_back().is_some_and(|c| c.is_alphabetic())
}

fn parse_resistance_euro(caps: &regex::Captures) -> (f64, String) {
    let int_part = &caps[1];
    let suffix = caps[2].to_uppercase();
    let frac_part = &caps[3];
    let value: f64 = format!("{int_part}.{frac_part}").parse().unwrap();
    match suffix.as_str() {
        "R" => (value, format!("{int_part}R{frac_part}")),
        "K" => (value * 1000.0, format!("{int_part}k{frac_part}")),
        "M" => (value * 1_000_000.0, format!("{int_part}M{frac_part}")),
        _ => (0.0, String::new()),
    }
}

fn parse_resistance_std(caps: &regex::Captures) -> (f64, String) {
    let value_str = &caps[1];
    let suffix = caps.get(2).map(|m| m.as_str().to_uppercase()).unwrap_or_default();
    let value: f64 = value_str.parse().unwrap();
    match suffix.as_str() {
        "R" | "OHM" => (value, format!("{value_str}Ohm")),
        "K" | "KOHM" => (value * 1000.0, format!("{value_str}kOhm")),
        "M" | "MOHM" => (value * 1_000_000.0, format!("{value_str}MOhm")),
        _ => (value, format!("{value_str}Ohm")),
    }
}

pub fn extract_values(query: &str) -> (Vec<ExtractedValue>, String) {
    let mut extractions: Vec<(usize, usize, ExtractedValue)> = Vec::new();
    let overlaps = |extractions: &[(usize, usize, ExtractedValue)], start: usize| {
        extractions.iter().any(|(s, e, _)| *s <= start && start < *e)
    };

    // Tolerance first (before other numbers)
    for caps in TOL.captures_iter(query) {
        let m = caps.get(0).unwrap();
        let pct: f64 = caps[1].parse().unwrap();
        extractions.push((m.start(), m.end(), ExtractedValue::new(m.as_str(), pct, "tolerance", format!("{}%", &caps[1]))));
    }

    // Frequency (before generic numbers)
    for caps in FREQ.captures_iter(query) {
        let m = caps.get(0).unwrap();
        let mut value: f64 = caps[1].parse().unwrap();
        let suffix = caps.get(2).map(|m| m.as_str().to_uppercase()).unwrap_or_default();
        let norm = match suffix.as_str() {
            "K" => { value *= 1e3; format!("{}kHz", &caps[1]) }
            "M" => { value *= 1e6; format!("{}MHz", &caps[1]) }
            "G" => { value *= 1e9; format!("{}GHz", &caps[1]) }
            _ => format!("{}Hz", &caps[1]),
        };
        extractions.push((m.start(), m.end(), ExtractedValue::new(m.as_str(), value, "frequency", norm)));
    }

    // Resistance (European notation first)
    for caps in RES_EURO.captures_iter(query) {
        let m = caps.get(0).unwrap();
        let (ohms, norm) = parse_resistance_euro(&caps);
        extractions.push((m.start(), m.end(), ExtractedValue::new(m.as_str(), ohms, "resistance", norm)));
    }

    // Resistance (standard) — skip if already matched by the European pattern
    for caps in RES_STD.captures_iter(query) {
        let m = caps.get(0).unwrap();
        if overlaps(&extractions, m.start()) {
            continue;
        }
        let (ohms, norm) = parse_resistance_std(&caps);
        extractions.push((m.start(), m.end(), ExtractedValue::new(m.as_str(), ohms, "resistance", norm)));
    }

    // Capacitance
    for caps in CAP.captures_iter(query) {
        let m = caps.get(0).unwrap();
        let value: f64 = caps[1].parse().unwrap();
        let suffix = caps[2].to_lowercase();
        let (farads, norm) = if suffix == "uf" || suffix == "f" {
            (value * 1e-6, format!("{}uF", &caps[1]))
        } else if suffix == "nf" {
            (value * 1e-9, format!("{}nF", &caps[1]))
        } else if suffix == "pf" {
            (value * 1e-12, format!("{}pF", &caps[1]))
        } else {
            (value, format!("{}F", &caps[1]))
        };
        extractions.push((m.start(), m.end(), ExtractedValue::new(m.as_str(), farads, "capacitance", norm)));
    }

    // Inductance
    for caps in IND.captures_iter(query) {
        let m = caps.get(0).unwrap();
        let value: f64 = caps[1].parse().unwrap();
        let suffix = caps[2].to_lowercase();
        let (henries, norm) = if suffix == "uh" || suffix == "h" {
            (value * 1e-6, format!("{}uH", &caps[1]))
        } else if suffix == "nh" {
            (value * 1e-9, format!("{}nH", &caps[1]))
        } else if suffix == "mh" {
            (value * 1e-3, format!("{}mH", &caps[1]))
        } else {
            (value, format!("{}H", &caps[1]))
        };
        extractions.push((m.start(), m.end(), ExtractedValue::new(m.as_str(), henries, "inductance", norm)));
    }

    // Voltage (careful not to match model numbers like STM32F103)
    for caps in VOLT.captures_iter(query) {
        let m = caps.get(0).unwrap();
        if preceded_by_letter(query, m.start()) {
            continue;
        }
        let mut value: f64 = caps[1].parse().unwrap();
        let norm = if caps.get(2).is_some() {
            value *= 1000.0;
            format!("{}kV", &caps[1])
        } else {
            format!("{}V", &caps[1])
        };
        extractions.push((m.start(), m.end(), ExtractedValue::new(m.as_str(), value, "voltage", norm)));
    }

    // Current
    for caps in CURR.captures_iter(query) {
        let m = caps.get(0).unwrap();
        if !preceded_by_start_or_space(query, m.start()) {
            continue;
        }
        let value: f64 = caps[1].parse().unwrap();
        let prefix = caps.get(2).map(|m| m.as_str().to_lowercase()).unwrap_or_default();
        let (amps, norm) = if prefix == "u" {
            (value * 1e-6, format!("{}uA", &caps[1]))
        } else if prefix == "m" {
            (value * 1e-3, format!("{}mA", &caps[1]))
        } else {
            (value, format!("{}A", &caps[1]))
        };
        extractions.push((m.start(), m.end(), ExtractedValue::new(m.as_str(), amps, "current", norm)));
    }

    // Power (fraction first)
    for caps in POWER_FRAC.captures_iter(query) {
        let m = caps.get(0).unwrap();
        let num: f64 = caps[1].parse().unwrap();
        let den: f64 = caps[2].parse().unwrap();
        extractions.push((m.start(), m.end(), ExtractedValue::new(m.as_str(), num / den, "power", format!("{}/{}W", &caps[1], &caps[2]))));
    }

    // Power (standard)
    for caps in POWER.captures_iter(query) {
        let m = caps.get(0).unwrap();
        if overlaps(&extractions, m.start()) {
            continue;
        }
        let value: f64 = caps[1].parse().unwrap();
        let prefix = caps.get(2).map(|m| m.as_str().to_lowercase()).unwrap_or_default();
        let (watts, norm) = if prefix == "m" {
            (value * 1e-3, format!("{}mW", &caps[1]))
        } else {
            (value, format!("{}W", &caps[1]))
        };
        extractions.push((m.start(), m.end(), ExtractedValue::new(m.as_str(), watts, "power", norm)));
    }

    // Pin count (normalize to "XP" format to match database values like "8P", "16P")
    for caps in PINS.captures_iter(query) {
        let m = caps.get(0).unwrap();
        let pins: i64 = caps[1].parse().unwrap();
        extractions.push((m.start(), m.end(), ExtractedValue::new(m.as_str(), pins as f64, "pin_count", format!("{pins}P"))));
    }

    // Position count (for connectors: 2-pos, 2 position, 2-way, 2P)
    for caps in POSITION.captures_iter(query) {
        let m = caps.get(0).unwrap();
        if overlaps(&extractions, m.start()) {
            continue;
        }
        let positions: i64 = caps[1].parse().unwrap();
        extractions.push((m.start(), m.end(), ExtractedValue::new(m.as_str(), positions as f64, "position_count", format!("{positions}P"))));
    }

    // Pin structure for headers (1x7, 2x20, etc.) — maps to "Pin Structure", not
    // "Number of Pins"
    for caps in PIN_STRUCTURE.captures_iter(query) {
        let m = caps.get(0).unwrap();
        if overlaps(&extractions, m.start()) {
            continue;
        }
        let rows: i64 = caps[1].parse().unwrap();
        let pins_per_row: i64 = caps[2].parse().unwrap();
        let total = rows * pins_per_row;
        extractions.push((m.start(), m.end(), ExtractedValue::new(m.as_str(), total as f64, "pin_structure", format!("{rows}x{pins_per_row}P"))));
    }

    // Pitch (connector spacing) — only extract known connector pitch values
    const COMMON_PITCHES: [f64; 12] = [0.5, 0.8, 1.0, 1.25, 1.27, 2.0, 2.54, 3.5, 3.81, 5.0, 5.08, 7.62];
    for caps in PITCH.captures_iter(query) {
        let m = caps.get(0).unwrap();
        if overlaps(&extractions, m.start()) {
            continue;
        }
        let pitch_val: f64 = caps[1].parse().unwrap();
        if COMMON_PITCHES.iter().any(|p| (*p - pitch_val).abs() < 1e-9) {
            extractions.push((m.start(), m.end(), ExtractedValue::new(m.as_str(), pitch_val, "pitch", format!("{}mm", &caps[1]))));
        }
    }

    // Dimensions — skip unreasonably large values (>100), which are display
    // resolutions like "128x64", not physical mm dimensions
    for caps in DIM.captures_iter(query) {
        let m = caps.get(0).unwrap();
        let x: f64 = caps[1].parse().unwrap();
        let y: f64 = caps[2].parse().unwrap();
        if x > 100.0 || y > 100.0 {
            continue;
        }
        extractions.push((m.start(), m.end(), ExtractedValue::new(m.as_str(), x * 1000.0 + y, "dimensions", format!("{}x{}mm", &caps[1], &caps[2]))));
    }

    // Sort by start position (Rust's `sort_by_key` is stable, matching Python's stable
    // `list.sort()` — ties keep the category-insertion order built above) and remove
    // overlaps by greedily keeping the first non-overlapping run.
    extractions.sort_by_key(|(start, _, _)| *start);
    let mut non_overlapping: Vec<(usize, usize, ExtractedValue)> = Vec::new();
    let mut last_end: usize = 0;
    for (start, end, val) in extractions {
        if start >= last_end {
            last_end = end;
            non_overlapping.push((start, end, val));
        }
    }

    let values: Vec<ExtractedValue> = non_overlapping.iter().map(|(_, _, v)| v.clone()).collect();

    let remaining = if non_overlapping.is_empty() {
        query.to_string()
    } else {
        let mut parts = Vec::new();
        let mut last_end = 0usize;
        for (start, end, _) in &non_overlapping {
            parts.push(&query[last_end..*start]);
            last_end = *end;
        }
        parts.push(&query[last_end..]);
        let joined = parts.join(" ");
        WHITESPACE_RE.replace_all(joined.trim(), " ").to_string()
    };

    (values, remaining)
}
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cd rust && cargo test -p pcbparts-smart-parser`
Expected: PASS — 13/13 tests.

- [ ] **Step 8: Commit**

```bash
git add rust/Cargo.toml rust/crates/pcbparts-smart-parser
git commit -m "rust: scaffold pcbparts-smart-parser crate, port smart_parser/values.py (characterization tests, no prior pytest coverage)"
```

---

### Task 2: `models.rs` + `packages.rs`

**Files:**
- Create: `rust/crates/pcbparts-smart-parser/src/models.rs`
- Create: `rust/crates/pcbparts-smart-parser/src/packages.rs`
- Modify: `rust/crates/pcbparts-smart-parser/src/lib.rs` (add `pub mod models;` and
  `pub mod packages;`)

**Interfaces:**
- Consumes: nothing from earlier tasks (both stdlib-only, matching Python).
- Produces: `extract_model_number(query: &str) -> (Option<String>, String)` and
  `extract_package(query: &str) -> (Option<String>, String, Option<String>)` — both
  consumed directly by Task 7's `parser.rs` (Steps 1 and 2 of `parse_smart_query`).

Both files have full existing pytest coverage in `tests/test_parsers.py`
(`TestModelNumberExtraction`, `TestModelNumberExcludesPackages`,
`TestPackageExtraction`) — ported 1:1, no characterization needed.

- [ ] **Step 1: Write the failing tests**

```rust
// rust/crates/pcbparts-smart-parser/src/models.rs — tests module (write this first)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_extraction() {
        for (query, expected_model) in [
            ("ESP32-S3-MINI-1", "ESP32-S3-MINI-1"),
            ("ESP32-S3-MINI", "ESP32-S3-MINI"),
            ("ESP32-C3-MINI", "ESP32-C3-MINI"),
            ("ESP32-S3-WROOM-1", "ESP32-S3-WROOM-1"),
            ("ESP32-S3-MINI-1-N8", "ESP32-S3-MINI-1-N8"),
            ("ESP32-C3", "ESP32-C3"),
            ("ESP32-S3", "ESP32-S3"),
            ("STM32F103C8T6", "STM32F103C8T6"),
            ("RP2040", "RP2040"),
            ("ATMEGA328P", "ATMEGA328P"),
            ("TP4056", "TP4056"),
            ("AMS1117", "AMS1117"),
            ("NE555", "NE555"),
        ] {
            let (model, _remaining) = extract_model_number(query);
            let model = model.unwrap_or_else(|| panic!("should extract model from '{query}'"));
            assert_eq!(model.to_uppercase(), expected_model.to_uppercase());
        }
    }

    #[test]
    fn esp32_mini_not_truncated() {
        let (model, remaining) = extract_model_number("ESP32-S3-MINI-1 module");
        assert_eq!(model, Some("ESP32-S3-MINI-1".to_string()));
        assert_eq!(remaining.trim(), "module");
    }

    #[test]
    fn package_not_detected_as_model() {
        const PACKAGE_LIKE: [&str; 8] =
            ["SOT", "SOD", "SOP", "SOIC", "QFN", "DFN", "TSSOP", "DIP"];
        for query in ["NPN SOT23", "diode SOD323", "driver QFN32", "ic TSSOP20", "mosfet SOIC8", "amp DIP8"] {
            let (model, _remaining) = extract_model_number(query);
            if let Some(model) = model {
                let model_upper = model.to_uppercase();
                for prefix in PACKAGE_LIKE {
                    let looks_like_package = model_upper.starts_with(prefix)
                        && model_upper.len() > prefix.len()
                        && model_upper[prefix.len()..].chars().all(|c| c.is_ascii_digit() || c == 'L');
                    assert!(!looks_like_package, "'{model}' looks like a package and should not be a model number");
                }
            }
        }
    }
}
```

```rust
// rust/crates/pcbparts-smart-parser/src/packages.rs — tests module (write this first)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_extraction() {
        for (query, expected_package, expected_remaining) in [
            ("30V N-Channel MOSFET SO-8", "SO-8", "30V N-Channel MOSFET"),
            ("mosfet SO8", "SO8", "mosfet"),
            ("SOP-8 mosfet", "SOP-8", "mosfet"),
            ("SOIC-8 driver", "SOIC-8", "driver"),
            ("10k resistor 0603", "0603", "10k resistor"),
            ("SOT-23 mosfet", "SOT-23", "mosfet"),
            ("QFN-24 mcu", "QFN-24", "mcu"),
            ("DIP-8 opamp", "DIP-8", "opamp"),
            ("NPN SOT23", "SOT-23", "NPN"),
            ("SOD323 diode", "SOD-323", "diode"),
            ("QFN32 mcu", "QFN32", "mcu"),
        ] {
            let (pkg, remaining, _suggested) = extract_package(query);
            let pkg = pkg.unwrap_or_else(|| panic!("should extract package from '{query}'"));
            assert_eq!(pkg.to_uppercase(), expected_package.to_uppercase());
            assert_eq!(remaining.trim(), expected_remaining.trim());
        }
    }

    #[test]
    fn usb_c_suggests_subcategory_without_becoming_package() {
        let (pkg, remaining, suggested) = extract_package("USB-C connector");
        assert_eq!(pkg, None);
        assert_eq!(remaining, "USB-C connector");
        assert_eq!(suggested, Some("usb connectors".to_string()));
    }

    #[test]
    fn sma_diode_package_matched_when_not_followed_by_connector() {
        // Regression case for the negative-lookahead workaround: bare "SMA" (a diode
        // package) is matched; "SMA connector" is not treated as the diode package.
        let (pkg, _remaining, _suggested) = extract_package("SMA diode 1A");
        assert_eq!(pkg, Some("SMA".to_string()));
    }

    #[test]
    fn sma_connector_not_matched_as_diode_package() {
        let (pkg, _remaining, _suggested) = extract_package("SMA connector coax");
        assert_ne!(pkg, Some("SMA".to_string()));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd rust && cargo test -p pcbparts-smart-parser models:: packages::`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

```rust
// rust/crates/pcbparts-smart-parser/src/models.rs — insert above the tests module
use regex::Regex;
use std::sync::LazyLock;

static MODEL_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // ESP32 compound module names must come first so the full compound name
        // (e.g. "ESP32-S3-MINI-1") matches before the generic pattern below truncates it.
        Regex::new(r"(?i)\b(ESP32(?:-[A-Z0-9]+)+)\b").unwrap(),
        Regex::new(r"(?i)\b(STM32[A-Z]\d+[A-Z0-9]*|RP2040|ATMEGA\d+[A-Z]*|PIC\d+[A-Z0-9]*)\b").unwrap(),
        Regex::new(r"(?i)\b(TP[45]\d{3}|AMS\d{4}|LM\d{4}|NE555|TL\d{3}|LMV?\d{3,4}|TPS\d{4,5})\b").unwrap(),
        Regex::new(r"(?i)\b(AO\d{4}|SI\d{4}|IRF\d{3,4}|IRLZ?\d{2,4}|2N\d{4}|BC\d{3})\b").unwrap(),
        Regex::new(r"(?i)\b(WS2812[A-Z]*|SK6812|APA102|TLC5940)\b").unwrap(),
        Regex::new(r"(?i)\b(1N\d{4}[A-Z]*|1SS\d{3}[A-Z]*|BAT\d{2}[A-Z]*|BAS\d{2}[A-Z]*|BAV\d{2}[A-Z]*)\b").unwrap(),
        // Generic IC model numbers (last resort) — intentionally last to avoid
        // truncating the more specific patterns above.
        Regex::new(r"(?i)\b([A-Z]{2,5}\d{2,5}[A-Z]?\d*(?:-[A-Z0-9]+)?)\b").unwrap(),
    ]
});

const PACKAGE_LIKE_PATTERNS: &[&str] = &[
    "SOT", "SOD", "SOP", "SOIC", "SSOP", "TSSOP", "MSOP", "QSOP",
    "QFN", "DFN", "QFP", "LQFP", "TQFP", "BGA", "DIP", "SIP",
    "CSP", "WLCSP", "LFCSP", "UCSP", "VCSP",
];

const AMBIGUOUS_WORDS: &[&str] = &[
    "LED", "LCD", "USB", "SPI", "I2C", "ADC", "DAC", "MCU", "CPU", "GPU",
    "RJ45", "RJ11", "RJ12", "RJ9", "RJ22", "RJ25",
];

/// Extract a likely model number from `query`. Returns `(model, remaining_query)`.
pub fn extract_model_number(query: &str) -> (Option<String>, String) {
    for pattern in MODEL_PATTERNS.iter() {
        let Some(caps) = pattern.captures(query) else { continue };
        let whole = caps.get(0).unwrap();
        let model = caps.get(1).unwrap().as_str();
        let model_upper = model.to_uppercase();

        if AMBIGUOUS_WORDS.contains(&model_upper.as_str()) {
            continue;
        }

        let mut is_package = false;
        for pkg_prefix in PACKAGE_LIKE_PATTERNS {
            if model_upper.starts_with(pkg_prefix) && model_upper.len() > pkg_prefix.len() {
                let rest = &model_upper[pkg_prefix.len()..];
                let rest_is_digits = !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit());
                let rest_is_digits_then_l = rest.chars().next().is_some_and(|c| c.is_ascii_digit())
                    && rest.chars().all(|c| c.is_ascii_digit() || c == 'L');
                if rest_is_digits || rest_is_digits_then_l {
                    is_package = true;
                    break;
                }
            }
        }
        if is_package {
            continue;
        }

        let remaining = format!("{}{}", &query[..whole.start()], &query[whole.end()..]);
        return (Some(model.to_string()), remaining.trim().to_string());
    }

    (None, query.to_string())
}
```

```rust
// rust/crates/pcbparts-smart-parser/src/packages.rs — insert above the tests module
use regex::Regex;
use std::sync::LazyLock;

static IMPERIAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(01005|0201|0402|0603|0805|1206|1210|1812|2010|2512)\b").unwrap());
static SMD_METRIC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(1610|1612|2012|2016|2520|2835|3014|3020|3030|3215|3225|3528|3535|5032|5050|5730|6035|7050|7060|8045|8080|9070)\b").unwrap()
});
static METRIC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(0402M|0603M|0805M|1206M)\b").unwrap());
static SOT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(SOT-?23(?:-\d+)?L?|SOT-?89(?:-\d+)?|SOT-?223(?:-\d+)?|SOT-?323(?:-\d+)?|SOT-?363(?:-\d+)?|SOT-?523(?:-\d+)?|SOT-?723(?:-\d+)?)\b").unwrap()
});
static SOD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(SOD-?(?:123|323|523|923|128|882|80|110|123FL|323FL))\b").unwrap()
});
static DO_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(DO-?(?:35|41|201|204|214|215|218|219|220)(?:AA|AB|AC|AD|AE|AF|AG)?)\b").unwrap()
});
static TO_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(TO-?92(?:S|L)?|TO-?220(?:F|FP|AB)?(?:-\d+)?|TO-?252(?:-\d+)?|TO-?263(?:-\d+)?|TO-?247(?:-\d+)?|TO-?251|TO-?3P(?:F)?|DPAK|D2PAK|D3PAK)\b").unwrap()
});
static QFN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b((?:V)?QFN-?\d+(?:-EP)?(?:\([^)]+\))?|DFN-?\d+(?:-EP)?(?:\([^)]+\))?|WQFN-?\d+|TQFN-?\d+|UQFN-?\d+)\b").unwrap()
});
static QFP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b((?:L|T|H|PQ)?QFP-?\d+(?:\([^)]+\))?)\b").unwrap());
static BGA_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b((?:FC|W|T|M|U|P|F)?BGA-?\d+(?:\([^)]+\))?)\b").unwrap());
static CSP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b((?:WL|LF|U|FC|V)?CSP-?\d+(?:-EP)?(?:\([^)]+\))?)\b").unwrap()
});
static DIP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b((?:P|S|SK|C)?DIP-?\d+(?:\([^)]+\))?|SIP-?\d+)\b").unwrap());
static TSSOP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(TSSOP-?\d+|SSOP-?\d+|MSOP-?\d+|QSOP-?\d+|HTSSOP-?\d+|VSSOP-?\d+)\b").unwrap()
});
static SOP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(SOP-?\d+(?:-\d+)?(?:\([^)]+\))?|SOIC-?\d+(?:-\d+)?(?:\([^)]+\))?)\b").unwrap()
});
static SO_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(SO-?\d+)\b").unwrap());
static MODULE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(SMD-?\d+|LGA-?\d+)\b").unwrap());
// Python: r'\b(SM[ABC])\b(?!\s*connector)' — the trailing negative lookahead has no
// `regex`-crate equivalent. Matched unconstrained here via DIODE_PKG_RE and filtered
// candidate-by-candidate in `find_diode_pkg`, which reproduces `re.search`'s
// leftmost-match-satisfying-the-whole-pattern behavior exactly.
static DIODE_PKG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(SM[ABC])\b").unwrap());
static FOLLOWED_BY_CONNECTOR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*connector").unwrap());
static MXX_DIODE_PKG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(M[478])\b").unwrap());
static USB_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(USB-?[ABC]|TYPE-?[ABC]|MICRO-?USB|MINI-?USB)\b").unwrap());

static SOT_HYPHEN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"SOT(\d)").unwrap());
static SOD_HYPHEN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"SOD(\d)").unwrap());
static TO_HYPHEN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"TO(\d)").unwrap());

fn find_diode_pkg(query: &str) -> Option<regex::Match<'_>> {
    DIODE_PKG_RE.find_iter(query).find(|m| !FOLLOWED_BY_CONNECTOR_RE.is_match(&query[m.end()..]))
}

/// Extract package from `query`. Returns `(package, remaining_query,
/// suggested_subcategory)` — `suggested_subcategory` is used for USB-C etc. where the
/// package pattern implies a component type rather than a literal package name.
pub fn extract_package(query: &str) -> (Option<String>, String, Option<String>) {
    // Every pattern below has exactly one capturing group whose span equals the whole
    // match (each pattern is `\b(...)\b` with nothing captured outside the group), so
    // `m.as_str()` on the whole-match `Match` is always the captured package text —
    // no separate `.captures()` call is needed.
    let candidates: [(fn(&str) -> Option<regex::Match<'_>>, &str); 19] = [
        (|q| IMPERIAL_RE.find(q), "imperial"),
        (|q| SMD_METRIC_RE.find(q), "smd_metric"),
        (|q| METRIC_RE.find(q), "metric"),
        (|q| SOT_RE.find(q), "sot"),
        (|q| SOD_RE.find(q), "sod"),
        (|q| DO_RE.find(q), "do"),
        (|q| TO_RE.find(q), "to"),
        (|q| QFN_RE.find(q), "qfn"),
        (|q| QFP_RE.find(q), "qfp"),
        (|q| BGA_RE.find(q), "bga"),
        (|q| CSP_RE.find(q), "csp"),
        (|q| DIP_RE.find(q), "dip"),
        (|q| TSSOP_RE.find(q), "tssop"),
        (|q| SOP_RE.find(q), "sop"),
        (|q| SO_RE.find(q), "so"),
        (|q| MODULE_RE.find(q), "module"),
        (find_diode_pkg, "diode_pkg"),
        (|q| MXX_DIODE_PKG_RE.find(q), "mxx_diode_pkg"),
        (|q| USB_RE.find(q), "usb"),
    ];

    for (find_fn, kind) in candidates {
        if let Some(m) = find_fn(query) {
            return finish_package_match(query, m, kind);
        }
    }
    (None, query.to_string(), None)
}

fn finish_package_match(query: &str, m: regex::Match<'_>, kind: &str) -> (Option<String>, String, Option<String>) {
    let mut package = m.as_str().to_uppercase();
    package = SOT_HYPHEN_RE.replace_all(&package, "SOT-$1").to_string();
    package = SOD_HYPHEN_RE.replace_all(&package, "SOD-$1").to_string();
    package = TO_HYPHEN_RE.replace_all(&package, "TO-$1").to_string();
    let remaining = format!("{}{}", &query[..m.start()], &query[m.end()..]).trim().to_string();

    if kind == "mxx_diode_pkg" {
        if package == "M4" || package == "M7" {
            package = "SMA".to_string();
        } else if package == "M8" {
            package = "SMB".to_string();
        }
    }

    if kind == "usb" {
        // USB-C/TYPE-C are not JLCPCB package names (their package is "SMD") — they're
        // connector types, so keep them in the query for FTS instead of using them as
        // a package filter.
        return (None, query.to_string(), Some("usb connectors".to_string()));
    }

    (Some(package), remaining, None)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd rust && cargo test -p pcbparts-smart-parser models:: packages::`
Expected: PASS — 6/6 tests (2 in `models.rs`, 4 in `packages.rs`).

- [ ] **Step 5: Commit**

```bash
git add rust/crates/pcbparts-smart-parser/src/models.rs rust/crates/pcbparts-smart-parser/src/packages.rs rust/crates/pcbparts-smart-parser/src/lib.rs
git commit -m "rust: port smart_parser/models.py and packages.py"
```

---

### Task 3: `connectors.rs`

**Files:**
- Create: `rust/crates/pcbparts-smart-parser/src/connectors.rs`
- Modify: `rust/crates/pcbparts-smart-parser/src/lib.rs` (add `pub mod connectors;`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `ConnectorSpec { series: Option<String>, pitch: Option<f64>, pins:
  Option<i64>, fts_term: Option<String> }`, `extract_connector_series(query: &str) ->
  (Option<ConnectorSpec>, String)`, `get_pitch_for_series(series: &str) -> Option<f64>`
  — `ConnectorSpec` and `extract_connector_series` are consumed by Task 7's `parser.rs`
  (`ParsedQuery::connector_spec` field, Step 2c).

Full existing pytest coverage: `TestConnectorSeriesExtraction` (`test_jst_series_extraction`,
`test_brand_alias_expansion`, `test_no_connector_series`).

- [ ] **Step 1: Write the failing tests**

```rust
// rust/crates/pcbparts-smart-parser/src/connectors.rs — tests module (write this first)
#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    #[test]
    fn jst_series_extraction() {
        for (query, expected_series, expected_pitch) in [
            ("jst sh 4-pin", "SH", 1.0),
            ("jst-sh connector", "SH", 1.0),
            ("JST SH 1mm 4P", "SH", 1.0),
            ("jst ph battery", "PH", 2.0),
            ("jst xh connector", "XH", 2.5),
            ("jst gh 6pin", "GH", 1.25),
            ("jst zh 1.5mm", "ZH", 1.5),
        ] {
            let (spec, _remaining) = extract_connector_series(query);
            let spec = spec.unwrap_or_else(|| panic!("should detect series in '{query}'"));
            assert_eq!(spec.series.as_deref(), Some(expected_series));
            approx(spec.pitch.unwrap(), expected_pitch);
        }
    }

    #[test]
    fn brand_alias_expansion() {
        for (query, expected_series, expected_pitch, expected_pins) in [
            ("qwiic connector", "SH", 1.0, Some(4)),
            ("Qwiic", "SH", 1.0, Some(4)),
            ("stemma qt", "SH", 1.0, Some(4)),
            ("STEMMA QT connector", "SH", 1.0, Some(4)),
            ("easyc connector", "SH", 1.0, Some(4)),
            ("easyC", "SH", 1.0, Some(4)),
            ("stemma connector", "PH", 2.0, None),
        ] {
            let (spec, _remaining) = extract_connector_series(query);
            let spec = spec.unwrap_or_else(|| panic!("should detect brand in '{query}'"));
            assert_eq!(spec.series.as_deref(), Some(expected_series));
            approx(spec.pitch.unwrap(), expected_pitch);
            assert_eq!(spec.pins, expected_pins);
        }
    }

    #[test]
    fn no_connector_series() {
        let (spec, remaining) = extract_connector_series("10k resistor 0603");
        assert_eq!(spec, None);
        assert_eq!(remaining, "10k resistor 0603");
    }

    #[test]
    fn get_pitch_for_series_known_and_unknown() {
        approx(get_pitch_for_series("SH").unwrap(), 1.0);
        approx(get_pitch_for_series("xh").unwrap(), 2.5);
        assert_eq!(get_pitch_for_series("ZZ"), None);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd rust && cargo test -p pcbparts-smart-parser connectors::`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

```rust
// rust/crates/pcbparts-smart-parser/src/connectors.rs — insert above the tests module
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorSpec {
    pub series: Option<String>,
    pub pitch: Option<f64>,
    pub pins: Option<i64>,
    pub fts_term: Option<String>,
}

impl ConnectorSpec {
    fn new(series: Option<&str>, pitch: Option<f64>, pins: Option<i64>, fts_term: Option<&str>) -> Self {
        Self { series: series.map(String::from), pitch, pins, fts_term: fts_term.map(String::from) }
    }
}

/// JST connector series with their pitch values (in mm), from JST datasheets.
pub fn jst_series_pitch() -> HashMap<&'static str, f64> {
    HashMap::from([
        ("sh", 1.0), ("sr", 1.0), ("gh", 1.25), ("zh", 1.5),
        ("pa", 2.0), ("ph", 2.0), ("eh", 2.5), ("xh", 2.5),
        ("vh", 3.96), ("vl", 6.2), ("bm", 1.0),
    ])
}

static JST_SERIES_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\bjst[\s-]*(sh|sr|gh|zh|pa|ph|eh|xh|vh|vl|bm)\b|\b(sh|sr|gh|zh|pa|ph|eh|xh|vh|vl|bm)\s*(?:series|connector|plug|socket|receptacle)\b",
    )
    .unwrap()
});
static STANDALONE_SERIES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(sh|gh|zh|ph|xh|vh|eh|pa)\b").unwrap());
static JST_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bjst\b").unwrap());
static WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// Brand aliases that map to specific JST connector specs — maker-ecosystem standards
/// that use JST SH connectors. A `Vec` (not a `HashMap`) preserves Python dict
/// insertion order exactly, since `extract_connector_series` returns on the first
/// substring match and order therefore affects results.
fn brand_connector_specs() -> Vec<(&'static str, ConnectorSpec)> {
    vec![
        ("qwiic", ConnectorSpec::new(Some("SH"), Some(1.0), Some(4), Some("SH"))),
        ("qwiic connector", ConnectorSpec::new(Some("SH"), Some(1.0), Some(4), Some("SH"))),
        ("stemma qt", ConnectorSpec::new(Some("SH"), Some(1.0), Some(4), Some("SH"))),
        ("stemmaqt", ConnectorSpec::new(Some("SH"), Some(1.0), Some(4), Some("SH"))),
        ("stemma", ConnectorSpec::new(Some("PH"), Some(2.0), None, Some("PH"))),
        ("easyc", ConnectorSpec::new(Some("SH"), Some(1.0), Some(4), Some("SH"))),
        ("easy c", ConnectorSpec::new(Some("SH"), Some(1.0), Some(4), Some("SH"))),
        ("grove", ConnectorSpec::new(None, Some(2.0), Some(4), Some("HY2.0"))),
    ]
}

/// Extract JST connector series and brand aliases from `query`. Returns
/// `(ConnectorSpec, remaining_query_with_series_removed)`.
pub fn extract_connector_series(query: &str) -> (Option<ConnectorSpec>, String) {
    let query_lower = query.to_lowercase();

    for (brand, spec) in brand_connector_specs() {
        if query_lower.contains(brand) {
            let pattern = Regex::new(&format!("(?i){}", regex::escape(brand))).unwrap();
            let mut remaining = pattern.replace_all(query, "").to_string();
            remaining = WHITESPACE_RE.replace_all(remaining.trim(), " ").to_string();
            return (Some(spec), remaining);
        }
    }

    if let Some(caps) = JST_SERIES_PATTERN.captures(query) {
        let m = caps.get(0).unwrap();
        let series = caps.get(1).or_else(|| caps.get(2)).unwrap().as_str().to_uppercase();
        let pitch = jst_series_pitch().get(series.to_lowercase().as_str()).copied();
        let mut remaining = format!("{}{}", &query[..m.start()], &query[m.end()..]);
        remaining = WHITESPACE_RE.replace_all(remaining.trim(), " ").to_string();
        return (Some(ConnectorSpec::new(Some(&series), pitch, None, Some(&series))), remaining);
    }

    if query_lower.contains("jst") {
        if let Some(m) = STANDALONE_SERIES.find(query) {
            let series = m.as_str().to_uppercase();
            let pitch = jst_series_pitch().get(series.to_lowercase().as_str()).copied();

            // Deliberate parity with a Python quirk (not exercised by any pytest case,
            // since every existing test hits the combined `jst sh`-style pattern
            // above instead): `series_match` is found against the ORIGINAL `query`,
            // but its `.start()`/`.end()` offsets are then applied to `remaining` — a
            // shorter string with "jst" already stripped out. Python's string slicing
            // clamps out-of-range indices instead of raising; the char-based slicing
            // below reproduces that same clamped behavior on `remaining` using the
            // stale offsets, rather than "fixing" it to re-search `remaining`.
            let jst_stripped = JST_WORD.replace_all(query, "").to_string();
            let start_char = query[..m.start()].chars().count();
            let end_char = query[..m.end()].chars().count();
            let stripped_chars: Vec<char> = jst_stripped.chars().collect();
            let len = stripped_chars.len();
            let s = start_char.min(len);
            let e = end_char.min(len).max(s);
            let mut remaining: String = stripped_chars[..s].iter().chain(stripped_chars[e..].iter()).collect();
            remaining = WHITESPACE_RE.replace_all(remaining.trim(), " ").to_string();

            return (Some(ConnectorSpec::new(Some(&series), pitch, None, Some(&series))), remaining);
        }
    }

    (None, query.to_string())
}

/// Get the pitch (in mm) for a JST series code like "SH", "PH", "XH".
pub fn get_pitch_for_series(series: &str) -> Option<f64> {
    jst_series_pitch().get(series.to_lowercase().as_str()).copied()
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd rust && cargo test -p pcbparts-smart-parser connectors::`
Expected: PASS — 4/4 tests.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/pcbparts-smart-parser/src/connectors.rs rust/crates/pcbparts-smart-parser/src/lib.rs
git commit -m "rust: port smart_parser/connectors.py"
```

---

### Task 4: `semantic.rs`

**Files:**
- Create: `rust/crates/pcbparts-smart-parser/src/semantic.rs`
- Modify: `rust/crates/pcbparts-smart-parser/src/lib.rs` (add `pub mod semantic;`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `SemanticFilter { spec_name: String, operator: &'static str, value: String,
  source: String }`, `extract_semantic_descriptors(query: &str) -> (Vec<SemanticFilter>,
  String)`, `remove_noise_words(query: &str) -> String`, `connector_noise_words() ->
  HashSet<&'static str>` — all four consumed by Task 7's `parser.rs` (Steps 5 and 7).

`remove_noise_words` has full pytest coverage (`TestNoiseWordRemoval`).
`extract_semantic_descriptors` has zero direct pytest coverage (only exercised
indirectly through `parse_smart_query`, which itself only covers 2 of the ~40
descriptor entries via `TestFerritBeadImpedance`/`TestConnectorParserIntegration`) — it
gets a characterization test instead, captured from the live Python function.

- [ ] **Step 1: Write the failing tests**

```rust
// rust/crates/pcbparts-smart-parser/src/semantic.rs — tests module (write this first)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_word_removal() {
        for (query, expected) in [
            ("USB-C receptacle", "USB-C"),
            ("USB-C jack", "USB-C"),
            ("USB-C plug", "USB-C"),
            ("resistor for power supply", "resistor power supply"),
            ("capacitor with high voltage", "capacitor high voltage"),
        ] {
            assert_eq!(remove_noise_words(query), expected, "'{query}' should become '{expected}'");
        }
    }

    #[test]
    fn extract_semantic_descriptors_characterization() {
        // Captured from the live Python `extract_semantic_descriptors`.
        let (filters, remaining) = extract_semantic_descriptors("low vgs mosfet");
        assert_eq!(remaining, "mosfet");
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].spec_name, "Vgs(th)");
        assert_eq!(filters[0].operator, "<");
        assert_eq!(filters[0].value, "2.5V");
        assert_eq!(filters[0].source, "low vgs");

        // Longest-match-first: "logic level" (11 chars) wins over any shorter
        // descriptor, and "n-channel" also matches separately in the same pass.
        let (filters, remaining) = extract_semantic_descriptors("n-channel logic level mosfet");
        assert_eq!(remaining, "mosfet");
        assert_eq!(filters.len(), 2);
        assert_eq!(filters[0].source, "logic level");
        assert_eq!(filters[0].spec_name, "Vgs(th)");
        assert_eq!(filters[1].source, "n-channel");
        assert_eq!(filters[1].spec_name, "Type");
        assert_eq!(filters[1].value, "N-Channel");

        let (filters, remaining) = extract_semantic_descriptors("bidirectional tvs");
        assert_eq!(remaining, "tvs");
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].spec_name, "Polarity");
        assert_eq!(filters[0].value, "Bidirectional");

        let (filters, remaining) = extract_semantic_descriptors("i2c sensor");
        assert_eq!(remaining, "sensor");
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].spec_name, "Interface");
        assert_eq!(filters[0].value, "I2C");

        // "blue" must not match inside "bluetooth" (word-boundary matching).
        let (filters, remaining) = extract_semantic_descriptors("bluetooth module");
        assert!(filters.is_empty());
        assert_eq!(remaining, "bluetooth module");

        let (filters, remaining) = extract_semantic_descriptors("red led high precision");
        assert_eq!(remaining, "led");
        assert_eq!(filters.len(), 2);
        assert_eq!(filters[0].source, "high precision");
        assert_eq!(filters[0].spec_name, "Tolerance");
        assert_eq!(filters[0].value, "0.05%");
        assert_eq!(filters[1].source, "red");
        assert_eq!(filters[1].spec_name, "Illumination Color");
        assert_eq!(filters[1].value, "Red");

        // "ultra low power" was removed from SEMANTIC_DESCRIPTORS (see the Python
        // source's comment on broken/unverified filters) — no match expected.
        let (filters, remaining) = extract_semantic_descriptors("ultra low power ldo");
        assert!(filters.is_empty());
        assert_eq!(remaining, "ultra low power ldo");
    }

    #[test]
    fn connector_noise_words_contains_expected() {
        let words = connector_noise_words();
        assert!(words.contains("power"));
        assert!(words.contains("male"));
        assert!(words.contains("female"));
        assert!(!words.contains("resistor"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd rust && cargo test -p pcbparts-smart-parser semantic::`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

```rust
// rust/crates/pcbparts-smart-parser/src/semantic.rs — insert above the tests module
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticFilter {
    pub spec_name: String,
    /// One of "=", ">=", "<=", ">", "<" — matches `pcbparts_search::spec_filter::SpecOperator`'s
    /// accepted strings, consumed via `SpecFilter::new` in Task 7.
    pub operator: &'static str,
    pub value: String,
    pub source: String,
}

impl SemanticFilter {
    fn new(spec_name: &str, operator: &'static str, value: &str, source: &str) -> Self {
        Self { spec_name: spec_name.to_string(), operator, value: value.to_string(), source: source.to_string() }
    }
}

/// Semantic descriptor mappings, in the exact order the Python `SEMANTIC_DESCRIPTORS`
/// dict literal declares them — order matters as a stable tie-break for descriptors of
/// equal length (see `SORTED_DESCRIPTORS` below).
fn semantic_descriptors() -> Vec<(&'static str, Vec<SemanticFilter>)> {
    vec![
        ("low vgs", vec![SemanticFilter::new("Vgs(th)", "<", "2.5V", "low vgs")]),
        ("low vgs(th)", vec![SemanticFilter::new("Vgs(th)", "<", "2.5V", "low vgs(th)")]),
        ("logic level", vec![SemanticFilter::new("Vgs(th)", "<", "2.5V", "logic level")]),
        ("logic-level", vec![SemanticFilter::new("Vgs(th)", "<", "2.5V", "logic-level")]),
        ("low threshold", vec![SemanticFilter::new("Vgs(th)", "<", "2.5V", "low threshold")]),
        ("low rds", vec![SemanticFilter::new("RDS(on)", "<", "50mOhm", "low rds")]),
        ("low rds(on)", vec![SemanticFilter::new("RDS(on)", "<", "50mOhm", "low rds(on)")]),
        ("low on-resistance", vec![SemanticFilter::new("RDS(on)", "<", "50mOhm", "low on-resistance")]),
        ("bidirectional", vec![SemanticFilter::new("Polarity", "=", "Bidirectional", "bidirectional")]),
        ("unidirectional", vec![SemanticFilter::new("Polarity", "=", "Unidirectional", "unidirectional")]),
        ("i2c", vec![SemanticFilter::new("Interface", "=", "I2C", "i2c")]),
        ("spi", vec![SemanticFilter::new("Interface", "=", "SPI", "spi")]),
        ("uart", vec![SemanticFilter::new("Interface", "=", "UART", "uart")]),
        ("i2s", vec![SemanticFilter::new("Interface", "=", "I2S", "i2s")]),
        ("can", vec![SemanticFilter::new("Interface", "=", "CAN", "can")]),
        ("rs485", vec![SemanticFilter::new("Interface", "=", "RS485", "rs485")]),
        ("rs232", vec![SemanticFilter::new("Interface", "=", "RS232", "rs232")]),
        ("1-wire", vec![SemanticFilter::new("Interface", "=", "Single-bus", "1-wire")]),
        ("one-wire", vec![SemanticFilter::new("Interface", "=", "Single-bus", "one-wire")]),
        ("single-bus", vec![SemanticFilter::new("Interface", "=", "Single-bus", "single-bus")]),
        ("n-channel", vec![SemanticFilter::new("Type", "=", "N-Channel", "n-channel")]),
        ("p-channel", vec![SemanticFilter::new("Type", "=", "P-Channel", "p-channel")]),
        ("n channel", vec![SemanticFilter::new("Type", "=", "N-Channel", "n channel")]),
        ("p channel", vec![SemanticFilter::new("Type", "=", "P-Channel", "p channel")]),
        ("nmos", vec![SemanticFilter::new("Type", "=", "N-Channel", "nmos")]),
        ("pmos", vec![SemanticFilter::new("Type", "=", "P-Channel", "pmos")]),
        ("npn", vec![SemanticFilter::new("Type", "=", "NPN", "npn")]),
        ("pnp", vec![SemanticFilter::new("Type", "=", "PNP", "pnp")]),
        ("red", vec![SemanticFilter::new("Illumination Color", "=", "Red", "red")]),
        ("green", vec![SemanticFilter::new("Illumination Color", "=", "Green", "green")]),
        ("blue", vec![SemanticFilter::new("Illumination Color", "=", "Blue", "blue")]),
        ("yellow", vec![SemanticFilter::new("Illumination Color", "=", "Yellow", "yellow")]),
        ("white", vec![SemanticFilter::new("Illumination Color", "=", "White", "white")]),
        ("orange", vec![SemanticFilter::new("Illumination Color", "=", "Orange", "orange")]),
        ("amber", vec![SemanticFilter::new("Illumination Color", "=", "Amber", "amber")]),
        ("c0g", vec![SemanticFilter::new("Temperature Coefficient", "=", "C0G", "c0g")]),
        ("np0", vec![SemanticFilter::new("Temperature Coefficient", "=", "NP0", "np0")]),
        ("x5r", vec![SemanticFilter::new("Temperature Coefficient", "=", "X5R", "x5r")]),
        ("x7r", vec![SemanticFilter::new("Temperature Coefficient", "=", "X7R", "x7r")]),
        ("x5s", vec![SemanticFilter::new("Temperature Coefficient", "=", "X5S", "x5s")]),
        ("x6s", vec![SemanticFilter::new("Temperature Coefficient", "=", "X6S", "x6s")]),
        ("x7s", vec![SemanticFilter::new("Temperature Coefficient", "=", "X7S", "x7s")]),
        ("y5v", vec![SemanticFilter::new("Temperature Coefficient", "=", "Y5V", "y5v")]),
        ("z5u", vec![SemanticFilter::new("Temperature Coefficient", "=", "Z5U", "z5u")]),
        ("fixed", vec![SemanticFilter::new("Output Type", "=", "Fixed", "fixed")]),
        ("adjustable", vec![SemanticFilter::new("Output Type", "=", "Adjustable", "adjustable")]),
        ("variable", vec![SemanticFilter::new("Output Type", "=", "Adjustable", "variable")]),
        ("precision", vec![SemanticFilter::new("Tolerance", "<=", "0.1%", "precision")]),
        ("high precision", vec![SemanticFilter::new("Tolerance", "<=", "0.05%", "high precision")]),
    ]
}

struct SortedDescriptor {
    pattern: Regex,
    filters: Vec<SemanticFilter>,
}

static SORTED_DESCRIPTORS: LazyLock<Vec<SortedDescriptor>> = LazyLock::new(|| {
    let mut entries = semantic_descriptors();
    // Stable sort, descending by length — Python's `sorted(..., key=len, reverse=True)`
    // is also stable and preserves original insertion order among equal-length keys
    // (Python's `reverse=True` reverses comparison sense, not final tie order).
    entries.sort_by(|(a, _), (b, _)| b.len().cmp(&a.len()));
    entries
        .into_iter()
        .map(|(key, filters)| SortedDescriptor {
            pattern: Regex::new(&format!(r"(?i)\b{}\b", regex::escape(key))).unwrap(),
            filters,
        })
        .collect()
});

static WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// Extract semantic descriptors from `query`. Returns `(filters, remaining_query)`.
pub fn extract_semantic_descriptors(query: &str) -> (Vec<SemanticFilter>, String) {
    let mut filters = Vec::new();
    let mut remaining = query.to_string();
    let mut query_lower = query.to_lowercase();

    for entry in SORTED_DESCRIPTORS.iter() {
        if entry.pattern.is_match(&query_lower) {
            filters.extend(entry.filters.iter().cloned());
            remaining = entry.pattern.replace_all(&remaining, "").trim().to_string();
            remaining = WHITESPACE_RE.replace_all(&remaining, " ").to_string();
            query_lower = remaining.to_lowercase();
        }
    }

    (filters, remaining)
}

/// Noise words to remove from queries.
pub fn noise_words() -> HashSet<&'static str> {
    HashSet::from([
        "for", "with", "and", "or", "the", "a", "an", "to", "in", "of",
        "type", "chip", "component", "part", "parts", "electronic", "electronics",
        "antenna",
        "receptacle", "jack", "plug", "socket",
    ])
}

/// Connector-specific noise words — only removed when a connector subcategory is
/// detected (JLCPCB descriptions don't consistently index gender/functionality terms).
pub fn connector_noise_words() -> HashSet<&'static str> {
    HashSet::from(["power", "data", "signal", "charging", "delivery", "pd", "male", "female"])
}

/// Remove common noise words from `query`.
pub fn remove_noise_words(query: &str) -> String {
    let noise = noise_words();
    query
        .split_whitespace()
        .filter(|w| !noise.contains(w.to_lowercase().as_str()))
        .collect::<Vec<_>>()
        .join(" ")
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd rust && cargo test -p pcbparts-smart-parser semantic::`
Expected: PASS — 3/3 tests.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/pcbparts-smart-parser/src/semantic.rs rust/crates/pcbparts-smart-parser/src/lib.rs
git commit -m "rust: port smart_parser/semantic.py (remove_noise_words tested, extract_semantic_descriptors characterization)"
```

---

### Task 5: `types.rs`

**Files:**
- Create: `rust/crates/pcbparts-smart-parser/src/types.rs`
- Modify: `rust/crates/pcbparts-smart-parser/src/lib.rs` (add `pub mod types;`)

**Interfaces:**
- Consumes: `pcbparts_parsers::subcategory_aliases::subcategory_aliases() ->
  HashMap<&'static str, &'static str>` (Phase 2A, confirmed by reading the actual
  committed `rust/crates/pcbparts-parsers/src/subcategory_aliases.rs`).
- Produces: `extract_component_type(query: &str) -> (Option<String>, String,
  Option<String>)`, `extract_mounting_type(query: &str) -> (Option<String>, String)` —
  both consumed by Task 7's `parser.rs` (Steps 2b and 3).

`types.py` has zero existing pytest coverage (confirmed: `pcbparts_mcp.smart_parser.types`
is never imported in `tests/test_parsers.py`) — characterization tests, captured from
the live Python functions.

- [ ] **Step 1: Write the failing tests**

```rust
// rust/crates/pcbparts-smart-parser/src/types.rs — tests module (write this first)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_component_type_characterization() {
        // Captured from the live Python `extract_component_type`.
        let (subcat, remaining, kw) = extract_component_type("10k resistor 0603");
        assert_eq!(subcat, Some("chip resistor - surface mount".to_string()));
        assert_eq!(remaining, "10k 0603");
        assert_eq!(kw, Some("resistor".to_string()));

        // Word-boundary matching: "sram" alone must match, but must not match inside
        // "psram" — this is the whole reason the Python source pre-sorts keywords by
        // length and wraps every match in `\b...\b`.
        let (subcat, remaining, kw) = extract_component_type("sram chip");
        assert_eq!(subcat, Some("sram".to_string()));
        assert_eq!(remaining, "chip");
        assert_eq!(kw, Some("sram".to_string()));

        let (subcat, remaining, kw) = extract_component_type("psram module");
        assert_eq!(subcat, None);
        assert_eq!(remaining, "psram module");
        assert_eq!(kw, None);

        let (subcat, remaining, kw) = extract_component_type("n-channel mosfet");
        assert_eq!(subcat, Some("mosfets".to_string()));
        assert_eq!(remaining, "");
        assert_eq!(kw, Some("n-channel mosfet".to_string()));

        let (subcat, remaining, kw) = extract_component_type("schottky diode");
        assert_eq!(subcat, Some("schottky diodes".to_string()));
        assert_eq!(remaining, "");
        assert_eq!(kw, Some("schottky diode".to_string()));

        let (subcat, remaining, kw) = extract_component_type("jst connector");
        assert_eq!(subcat, Some("wire to board connector".to_string()));
        assert_eq!(remaining, "");
        assert_eq!(kw, Some("jst connector".to_string()));

        let (subcat, remaining, kw) = extract_component_type("unknown widget xyz");
        assert_eq!(subcat, None);
        assert_eq!(remaining, "unknown widget xyz");
        assert_eq!(kw, None);
    }

    #[test]
    fn extract_mounting_type_characterization() {
        // Captured from the live Python `extract_mounting_type`.
        assert_eq!(extract_mounting_type("PTH resistor"), (Some("Through Hole".to_string()), "resistor".to_string()));
        assert_eq!(
            extract_mounting_type("through-hole capacitor"),
            (Some("Through Hole".to_string()), "capacitor".to_string())
        );
        assert_eq!(extract_mounting_type("SMD resistor"), (Some("SMD".to_string()), "resistor".to_string()));
        assert_eq!(extract_mounting_type("SMT capacitor"), (Some("SMD".to_string()), "capacitor".to_string()));
        assert_eq!(extract_mounting_type("leaded diode"), (Some("Through Hole".to_string()), "diode".to_string()));
        assert_eq!(extract_mounting_type("no hint here"), (None, "no hint here".to_string()));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd rust && cargo test -p pcbparts-smart-parser types::`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

```rust
// rust/crates/pcbparts-smart-parser/src/types.rs — insert above the tests module
use pcbparts_parsers::subcategory_aliases::subcategory_aliases;
use regex::Regex;
use std::sync::LazyLock;

// Pre-sorted by length (longest first) for correct matching — a stable sort, matching
// Python's `sorted(SUBCATEGORY_ALIASES.keys(), key=len, reverse=True)`.
static SUBCATEGORY_KEYWORDS_BY_LENGTH: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut keys: Vec<&'static str> = subcategory_aliases().into_keys().collect();
    keys.sort_by(|a, b| b.len().cmp(&a.len()));
    keys
});

static WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// Extract component type from `query`. Returns `(subcategory_name, remaining_query,
/// matched_keyword)`.
pub fn extract_component_type(query: &str) -> (Option<String>, String, Option<String>) {
    let query_lower = query.to_lowercase();
    let aliases = subcategory_aliases();

    for &keyword in SUBCATEGORY_KEYWORDS_BY_LENGTH.iter() {
        // Word boundaries avoid "sram" matching inside "PSRAM".
        let pattern = Regex::new(&format!(r"(?i)\b{}\b", regex::escape(keyword))).unwrap();
        if pattern.is_match(&query_lower) {
            let remaining = pattern.replace_all(query, "").trim().to_string();
            let remaining = WHITESPACE_RE.replace_all(&remaining, " ").to_string();
            return (Some(aliases[keyword].to_string()), remaining, Some(keyword.to_string()));
        }
    }

    (None, query.to_string(), None)
}

// Mounting type patterns: PTH/THT -> Through Hole, SMD/SMT -> SMD.
static MOUNTING_TYPE_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"(?i)\b(PTH|THT|through[- ]?hole|leaded)\b").unwrap(), "Through Hole"),
        (Regex::new(r"(?i)\b(SMD|SMT|surface[- ]?mount)\b").unwrap(), "SMD"),
    ]
});

/// Extract mounting type from `query`. Returns `(mounting_type, remaining_query)`
/// where `mounting_type` is `"SMD"`, `"Through Hole"`, or `None`.
pub fn extract_mounting_type(query: &str) -> (Option<String>, String) {
    for (pattern, mount_type) in MOUNTING_TYPE_PATTERNS.iter() {
        if let Some(m) = pattern.find(query) {
            let remaining = format!("{}{}", &query[..m.start()], &query[m.end()..]);
            let remaining = WHITESPACE_RE.replace_all(&remaining, " ").trim().to_string();
            return (Some(mount_type.to_string()), remaining);
        }
    }
    (None, query.to_string())
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd rust && cargo test -p pcbparts-smart-parser types::`
Expected: PASS — 2/2 tests.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/pcbparts-smart-parser/src/types.rs rust/crates/pcbparts-smart-parser/src/lib.rs
git commit -m "rust: port smart_parser/types.py (characterization tests, no prior pytest coverage)"
```

---

### Task 6: `mapping.rs`

**Files:**
- Create: `rust/crates/pcbparts-smart-parser/src/mapping.rs`
- Modify: `rust/crates/pcbparts-smart-parser/src/lib.rs` (add `pub mod mapping;`)

**Interfaces:**
- Consumes: `crate::values::ExtractedValue` (Task 1).
- Produces: `map_value_to_spec(value: &ExtractedValue, component_type: Option<&str>,
  matched_keyword: Option<&str>) -> (String, &'static str)`,
  `infer_subcategory_from_values(values: &[ExtractedValue]) -> Option<String>` — both
  consumed by Task 7's `parser.rs` (Steps 4b and 6).

`mapping.py` has zero existing pytest coverage — characterization tests, captured from
the live Python functions.

- [ ] **Step 1: Write the failing tests**

```rust
// rust/crates/pcbparts-smart-parser/src/mapping.rs — tests module (write this first)
#[cfg(test)]
mod tests {
    use super::*;

    fn value(raw: &str, val: f64, unit_type: &str, normalized: &str) -> ExtractedValue {
        ExtractedValue { raw: raw.to_string(), value: val, unit_type: unit_type.to_string(), normalized: normalized.to_string() }
    }

    #[test]
    fn map_value_to_spec_characterization() {
        // Captured from the live Python `map_value_to_spec`.
        let v1 = value("100V", 100.0, "voltage", "100V");
        assert_eq!(map_value_to_spec(&v1, Some("mosfets"), Some("mosfet")), ("Vds".to_string(), ">="));

        let v2 = value("2A", 2.0, "current", "2A");
        assert_eq!(
            map_value_to_spec(&v2, Some("switching diodes"), Some("schottky diode")),
            ("If".to_string(), ">=")
        );

        let v3 = value("10k", 10000.0, "resistance", "10kOhm");
        assert_eq!(map_value_to_spec(&v3, None, None), ("Resistance".to_string(), "="));

        let v4 = value("16", 16.0, "pin_count", "16P");
        assert_eq!(map_value_to_spec(&v4, Some("connectors"), Some("header")), ("Number of Pins".to_string(), "="));

        // Falls through every mapping table to Python's `str.title()` on the unit
        // type: uppercase the first letter of each alphabetic run, lowercase the
        // rest — underscores are NOT word separators to `.title()`, only the
        // alpha/non-alpha transition is.
        let v5 = value("xyz", 1.0, "totally_unknown_type", "xyz");
        assert_eq!(map_value_to_spec(&v5, None, None), ("Totally_Unknown_Type".to_string(), "="));
    }

    #[test]
    fn infer_subcategory_from_values_characterization() {
        // Captured from the live Python `infer_subcategory_from_values`.
        assert_eq!(
            infer_subcategory_from_values(&[value("10k", 10000.0, "resistance", "10kOhm")]),
            Some("chip resistor - surface mount".to_string())
        );
        assert_eq!(
            infer_subcategory_from_values(&[value("10uF", 1e-5, "capacitance", "10uF")]),
            Some("multilayer ceramic capacitors mlcc - smd/smt".to_string())
        );
        // Inductance alone infers nothing — inductors are split across multiple
        // subcategories (Inductors (SMD), Power Inductors, ...), so text search is
        // left to cover all of them instead of guessing one.
        assert_eq!(infer_subcategory_from_values(&[value("10uH", 1e-5, "inductance", "10uH")]), None);
        // Capacitance takes priority when both resistance and capacitance are present.
        assert_eq!(
            infer_subcategory_from_values(&[
                value("10k", 10000.0, "resistance", "10kOhm"),
                value("10uF", 1e-5, "capacitance", "10uF"),
            ]),
            Some("multilayer ceramic capacitors mlcc - smd/smt".to_string())
        );
        assert_eq!(infer_subcategory_from_values(&[]), None);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd rust && cargo test -p pcbparts-smart-parser mapping::`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

```rust
// rust/crates/pcbparts-smart-parser/src/mapping.rs — insert above the tests module
use crate::values::ExtractedValue;
use std::collections::{HashMap, HashSet};

/// Maps (category_keyword, value_type) -> spec_name, so a bare "voltage" value maps to
/// "Vds" for MOSFETs, "Vr" for diodes, "Output Voltage" for regulators, etc.
pub fn category_attribute_map() -> HashMap<&'static str, HashMap<&'static str, &'static str>> {
    let mut m: HashMap<&'static str, HashMap<&'static str, &'static str>> = HashMap::new();
    let vds_id = HashMap::from([("voltage", "Vds"), ("current", "Id")]);
    for key in ["mosfet", "mosfets", "n-channel mosfet", "p-channel mosfet", "nmos", "pmos"] {
        m.insert(key, vds_id.clone());
    }
    let vr_if = HashMap::from([("voltage", "Vr"), ("current", "If")]);
    for key in ["diode", "schottky", "schottky diode", "rectifier", "rectifier diode"] {
        m.insert(key, vr_if.clone());
    }
    let zener = HashMap::from([("voltage", "Zener Voltage(Nom)")]);
    m.insert("zener", zener.clone());
    m.insert("zener diode", zener);
    let tvs = HashMap::from([
        ("voltage", "Reverse Stand-Off Voltage (Vrwm)"),
        ("current", "Peak Pulse Current (Ipp)"),
    ]);
    m.insert("tvs", tvs.clone());
    m.insert("tvs diode", tvs);
    let inductor_current = HashMap::from([("current", "Current Rating")]);
    for key in ["inductor", "inductors", "power inductor", "coil"] {
        m.insert(key, inductor_current.clone());
    }
    let ferrite = HashMap::from([("current", "Current Rating"), ("resistance", "Impedance @ Frequency")]);
    for key in ["ferrite bead", "ferrite beads", "ferrite"] {
        m.insert(key, ferrite.clone());
    }
    let cap_voltage = HashMap::from([("voltage", "Voltage Rating")]);
    for key in ["capacitor", "capacitors", "mlcc", "tantalum"] {
        m.insert(key, cap_voltage.clone());
    }
    m.insert("electrolytic", HashMap::from([("voltage", "Voltage Rating"), ("current", "Ripple Current")]));
    let crystal_freq = HashMap::from([("frequency", "Frequency")]);
    for key in ["crystal", "crystals", "oscillator"] {
        m.insert(key, crystal_freq.clone());
    }
    let vceo_ic = HashMap::from([("voltage", "Vceo"), ("current", "Ic")]);
    for key in ["bjt", "transistor", "npn", "pnp"] {
        m.insert(key, vceo_ic.clone());
    }
    let charger = HashMap::from([("current", "Charge Current - Max"), ("voltage", "Charging Saturation Voltage")]);
    for key in ["battery charger", "lipo charger", "lithium charger", "battery management", "charging ic"] {
        m.insert(key, charger.clone());
    }
    let regulator = HashMap::from([("voltage", "Output Voltage"), ("current", "Output Current")]);
    for key in ["ldo", "regulator", "linear regulator", "buck", "boost", "dc-dc"] {
        m.insert(key, regulator.clone());
    }
    let led = HashMap::from([("current", "Forward Current"), ("voltage", "Voltage - Forward(Vf)")]);
    m.insert("led", led.clone());
    m.insert("leds", led);
    let fuse = HashMap::from([("voltage", "Voltage - Max"), ("current", "Hold Current")]);
    for key in ["fuse", "ptc", "resettable fuse"] {
        m.insert(key, fuse.clone());
    }
    let usb_contacts = HashMap::from([
        ("pin_count", "Number of Contacts"),
        ("pitch", "Pitch"),
        ("position_count", "Number of Contacts"),
    ]);
    for key in ["usb connector", "usb connectors"] {
        m.insert(key, usb_contacts.clone());
    }
    let usb_c_contacts = HashMap::from([("pin_count", "Number of Contacts"), ("position_count", "Number of Contacts")]);
    for key in ["usb-c", "type-c"] {
        m.insert(key, usb_c_contacts.clone());
    }
    let pins = HashMap::from([("pin_count", "Number of Pins"), ("pitch", "Pitch"), ("position_count", "Number of Pins")]);
    for key in ["connector", "header", "pin header", "pin headers", "jst", "wire to board connector"] {
        m.insert(key, pins.clone());
    }
    let positions = HashMap::from([
        ("pin_count", "Number of Positions"),
        ("pitch", "Pitch"),
        ("position_count", "Number of Positions"),
    ]);
    for key in ["female header", "female headers"] {
        m.insert(key, positions.clone());
    }
    let terminal = HashMap::from([
        ("pin_count", "Number of Pins"),
        ("pitch", "Pitch"),
        ("position_count", "Number of Pins"),
        ("voltage", "Voltage Rating (Max)"),
        ("current", "Current Rating"),
    ]);
    for key in ["terminal block", "screw terminal", "screw terminal blocks", "pluggable system terminal block"] {
        m.insert(key, terminal.clone());
    }
    let idc = HashMap::from([
        ("pin_count", "Number of Positions or Pins"),
        ("pitch", "Pitch"),
        ("position_count", "Number of Positions or Pins"),
    ]);
    for key in ["idc connector", "idc connectors"] {
        m.insert(key, idc.clone());
    }
    let ffc_fpc = HashMap::from([("pin_count", "Number of Contacts"), ("pitch", "Pitch"), ("position_count", "Number of Contacts")]);
    for key in ["ffc", "fpc"] {
        m.insert(key, ffc_fpc.clone());
    }
    m
}

const NUMERIC_LIKE_UNIT_TYPES: &[&str] = &[
    "resistance", "capacitance", "inductance", "frequency", "tolerance",
    "pin_count", "position_count", "pin_structure", "pitch",
];

fn default_specs() -> HashMap<&'static str, (&'static str, &'static str)> {
    HashMap::from([
        ("voltage", ("Voltage Rating", ">=")),
        ("current", ("Current Rating", ">=")),
        ("resistance", ("Resistance", "=")),
        ("capacitance", ("Capacitance", "=")),
        ("inductance", ("Inductance", "=")),
        ("frequency", ("Frequency", "=")),
        ("tolerance", ("Tolerance", "=")),
        ("power", ("Power", ">=")),
        ("pin_count", ("Number of Pins", "=")),
        ("position_count", ("Number of Pins", "=")),
        ("pin_structure", ("Pin Structure", "=")),
        ("pitch", ("Pitch", "=")),
    ])
}

/// Python's `str.title()`: uppercase the first letter of every run of alphabetic
/// characters, lowercase the rest — underscores/digits/punctuation are not word
/// separators, only the alpha/non-alpha transition is (e.g. "totally_unknown_type" ->
/// "Totally_Unknown_Type").
fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_is_alpha = false;
    for c in s.chars() {
        if c.is_alphabetic() {
            if prev_is_alpha {
                out.extend(c.to_lowercase());
            } else {
                out.extend(c.to_uppercase());
            }
            prev_is_alpha = true;
        } else {
            out.push(c);
            prev_is_alpha = false;
        }
    }
    out
}

/// Map an extracted value to the appropriate spec name based on context. Returns
/// `(spec_name, operator)`.
pub fn map_value_to_spec(
    value: &ExtractedValue,
    component_type: Option<&str>,
    matched_keyword: Option<&str>,
) -> (String, &'static str) {
    let cat_map = category_attribute_map();

    for candidate in [matched_keyword, component_type] {
        if let Some(kw) = candidate {
            let kw_lower = kw.to_lowercase();
            if let Some(entry) = cat_map.get(kw_lower.as_str()) {
                if let Some(&spec_name) = entry.get(value.unit_type.as_str()) {
                    let op = if NUMERIC_LIKE_UNIT_TYPES.contains(&value.unit_type.as_str()) { "=" } else { ">=" };
                    return (spec_name.to_string(), op);
                }
            }
        }
    }

    if let Some(&(spec_name, op)) = default_specs().get(value.unit_type.as_str()) {
        return (spec_name.to_string(), op);
    }

    (title_case(&value.unit_type), "=")
}

/// Infer likely subcategory from extracted values, used when no explicit component
/// type is specified.
pub fn infer_subcategory_from_values(values: &[ExtractedValue]) -> Option<String> {
    let value_types: HashSet<&str> = values.iter().map(|v| v.unit_type.as_str()).collect();

    if value_types.contains("resistance") && !value_types.contains("inductance") && !value_types.contains("capacitance") {
        return Some("chip resistor - surface mount".to_string());
    }
    if value_types.contains("capacitance") {
        return Some("multilayer ceramic capacitors mlcc - smd/smt".to_string());
    }
    None
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd rust && cargo test -p pcbparts-smart-parser mapping::`
Expected: PASS — 2/2 tests.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/pcbparts-smart-parser/src/mapping.rs rust/crates/pcbparts-smart-parser/src/lib.rs
git commit -m "rust: port smart_parser/mapping.py (characterization tests, no prior pytest coverage)"
```

---

### Task 7: `parser.rs` + `lib.rs` final integration

**Files:**
- Create: `rust/crates/pcbparts-smart-parser/src/parser.rs`
- Modify: `rust/crates/pcbparts-smart-parser/src/lib.rs` (add `pub mod parser;` plus the
  full `pub use` re-export list mirroring `smart_parser/__init__.py`'s `__all__`)

**Interfaces:**
- Consumes: `crate::connectors::{ConnectorSpec, extract_connector_series}` (Task 3),
  `crate::mapping::{infer_subcategory_from_values, map_value_to_spec}` (Task 6),
  `crate::models::extract_model_number` (Task 2), `crate::packages::extract_package`
  (Task 2), `crate::semantic::{connector_noise_words, extract_semantic_descriptors,
  remove_noise_words}` (Task 4), `crate::types::{extract_component_type,
  extract_mounting_type}` (Task 5), `crate::values::{ExtractedValue, extract_values}`
  (Task 1), and `pcbparts_search::spec_filter::SpecFilter` (`SpecFilter::new(name:
  impl Into<String>, operator: &str, value: impl Into<String>) -> Result<Self, String>`,
  Phase 3, confirmed by reading the actual committed
  `rust/crates/pcbparts-search/src/spec_filter.rs`).
- Produces: `ParsedQuery { original: String, remaining_text: String, subcategory:
  Option<String>, spec_filters: Vec<SpecFilter>, package: Option<String>, model_number:
  Option<String>, mounting_type: Option<String>, connector_spec: Option<ConnectorSpec>,
  detected: serde_json::Value }`, `parse_smart_query(query: &str) -> ParsedQuery`,
  `merge_spec_filters(manual_filters: Option<Vec<SpecFilter>>, auto_filters:
  Option<Vec<SpecFilter>>) -> Option<Vec<SpecFilter>>` — this is the crate's public API
  surface that Phase 5 (`pcbparts-db`'s component half) and eventually Phase 9
  (`pcbparts-server`'s `smart_search`-style MCP tool) will call.

`parse_smart_query` has partial pytest coverage: `TestFerritBeadImpedance` (5 cases) and
`TestConnectorParserIntegration` (3 cases) from `tests/test_parsers.py` — ported 1:1.
The rest of the function's ~15 internal branches have no dedicated pytest coverage, so
this task adds characterization tests for five representative end-to-end queries,
captured from the live Python `parse_smart_query`. `merge_spec_filters` has zero pytest
coverage — characterization only.

**Note on `subcategory` vs. `result.subcategory`:** as flagged in Global Constraints,
Step 3's local `subcategory` binding and the `result.subcategory` field diverge after
Step 4b/4c can reassign the latter. The implementation below keeps both bindings alive
throughout the function and reads from whichever one the corresponding line in
`parser.py` actually reads from — verified line-by-line against the Python source, not
assumed.

- [ ] **Step 1: Write the failing tests**

```rust
// rust/crates/pcbparts-smart-parser/src/parser.rs — tests module (write this first)
#[cfg(test)]
mod tests {
    use super::*;

    // --- TestFerritBeadImpedance (tests/test_parsers.py) ---
    #[test]
    fn ferrite_bead_impedance_parsing() {
        for (query, expected_impedance) in [
            ("30 ohm ferrite bead 0603", "30Ohm"),
            ("ferrite bead 0603 30", "30Ohm"),
            ("ferrite bead 100 0402", "100Ohm"),
            ("120 ferrite bead", "120Ohm"),
            ("600 ohm ferrite 0603", "600Ohm"),
        ] {
            let result = parse_smart_query(query);
            assert_eq!(result.subcategory.as_deref(), Some("ferrite beads"), "query: {query}");
            let impedance_filters: Vec<_> = result.spec_filters.iter().filter(|f| f.name.contains("Impedance")).collect();
            assert_eq!(impedance_filters.len(), 1, "query: {query}, filters: {:?}", result.spec_filters);
            assert_eq!(impedance_filters[0].value, expected_impedance, "query: {query}");
        }
    }

    // --- TestConnectorParserIntegration (tests/test_parsers.py) ---
    #[test]
    fn jst_sh_4pin_adds_connector_spec() {
        let result = parse_smart_query("jst sh 4-pin");
        assert_eq!(result.subcategory.as_deref(), Some("wire to board connector"));
        let cs = result.connector_spec.expect("connector_spec should be set");
        assert_eq!(cs.series.as_deref(), Some("SH"));
        assert_eq!(cs.pitch, Some(1.0));
        assert!(result.remaining_text.to_lowercase().contains("sh"));
    }

    #[test]
    fn qwiic_expands_to_jst_sh() {
        let result = parse_smart_query("qwiic connector");
        assert_eq!(result.subcategory.as_deref(), Some("wire to board connector"));
        let cs = result.connector_spec.expect("connector_spec should be set");
        assert_eq!(cs.series.as_deref(), Some("SH"));
        assert_eq!(cs.pitch, Some(1.0));
        assert_eq!(cs.pins, Some(4));
        assert!(result.remaining_text.to_lowercase().contains("sh"));
    }

    #[test]
    fn easyc_same_as_qwiic() {
        let result = parse_smart_query("easyc");
        assert_eq!(result.subcategory.as_deref(), Some("wire to board connector"));
        let cs = result.connector_spec.expect("connector_spec should be set");
        assert_eq!(cs.series.as_deref(), Some("SH"));
        assert_eq!(cs.pitch, Some(1.0));
        assert_eq!(cs.pins, Some(4));
    }

    // --- Characterization: representative end-to-end queries, captured from the live
    // Python `parse_smart_query` (no dedicated pytest coverage for these shapes) ---
    #[test]
    fn characterization_resistor_with_package_and_tolerance() {
        let r = parse_smart_query("10k resistor 0603 1%");
        assert_eq!(r.subcategory.as_deref(), Some("chip resistor - surface mount"));
        assert_eq!(r.package.as_deref(), Some("0603"));
        assert_eq!(r.model_number, None);
        assert_eq!(r.remaining_text, "");
        assert_eq!(r.spec_filters.len(), 2);
        assert_eq!(r.spec_filters[0].name, "Resistance");
        assert_eq!(r.spec_filters[0].value, "10kOhm");
        assert_eq!(r.spec_filters[1].name, "Tolerance");
        assert_eq!(r.spec_filters[1].value, "1%");
    }

    #[test]
    fn characterization_mosfet_voltage_maps_to_vds() {
        let r = parse_smart_query("100V mosfet");
        assert_eq!(r.subcategory.as_deref(), Some("mosfets"));
        assert_eq!(r.package, None);
        assert_eq!(r.remaining_text, "");
        assert_eq!(r.spec_filters.len(), 1);
        assert_eq!(r.spec_filters[0].name, "Vds");
        assert_eq!(r.spec_filters[0].value, "100V");
    }

    #[test]
    fn characterization_model_number_becomes_only_fts_term() {
        let r = parse_smart_query("TP4056 lithium battery charger");
        assert_eq!(r.subcategory.as_deref(), Some("battery management"));
        assert_eq!(r.model_number.as_deref(), Some("TP4056"));
        // A model number is present, so remaining_text is ONLY the model — not the
        // rest of the descriptive text (Python's Step 8: "Search only for the model
        // number" for precision).
        assert_eq!(r.remaining_text, "TP4056");
        assert!(r.spec_filters.is_empty());
    }

    #[test]
    fn characterization_n_channel_keyword_and_low_vgs_semantic() {
        let r = parse_smart_query("n-channel mosfet low Vgs");
        assert_eq!(r.subcategory.as_deref(), Some("mosfets"));
        assert_eq!(r.remaining_text, "");
        assert_eq!(r.spec_filters.len(), 2);
        assert_eq!(r.spec_filters[0].name, "Type");
        assert_eq!(r.spec_filters[0].value, "N-Channel");
        assert_eq!(r.spec_filters[1].name, "Vgs(th)");
        assert_eq!(r.spec_filters[1].value, "2.5V");
    }

    #[test]
    fn characterization_inductor_current_maps_to_current_rating() {
        let r = parse_smart_query("10uH inductor 2A");
        assert_eq!(r.subcategory.as_deref(), Some("inductors (smd)"));
        assert_eq!(r.remaining_text, "");
        assert_eq!(r.spec_filters.len(), 2);
        assert_eq!(r.spec_filters[0].name, "Inductance");
        assert_eq!(r.spec_filters[0].value, "10uH");
        assert_eq!(r.spec_filters[1].name, "Current Rating");
        assert_eq!(r.spec_filters[1].value, "2A");
    }

    // --- merge_spec_filters: zero pytest coverage, characterization only ---
    #[test]
    fn merge_spec_filters_manual_takes_precedence_case_insensitively() {
        let manual = vec![SpecFilter::new("Resistance", "=", "10kOhm").unwrap()];
        let auto = vec![
            SpecFilter::new("resistance", ">=", "5kOhm").unwrap(),
            SpecFilter::new("Tolerance", "=", "1%").unwrap(),
        ];
        let merged = merge_spec_filters(Some(manual), Some(auto)).unwrap();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].name, "Resistance");
        assert_eq!(merged[0].value, "10kOhm"); // manual value wins, not "5kOhm"
        assert_eq!(merged[1].name, "Tolerance");
    }

    #[test]
    fn merge_spec_filters_none_and_empty_cases() {
        assert_eq!(merge_spec_filters(None, None), None);

        let manual = vec![SpecFilter::new("X", "=", "1").unwrap()];
        let merged = merge_spec_filters(Some(manual.clone()), None).unwrap();
        assert_eq!(merged, manual);

        let auto = vec![SpecFilter::new("X", "=", "1").unwrap()];
        let merged = merge_spec_filters(None, Some(auto.clone())).unwrap();
        assert_eq!(merged, auto);

        // Both present but empty: Python's `if not auto_filters: return
        // manual_filters` fires first (an empty list is falsy), returning
        // manual_filters unchanged — i.e. also empty, not None.
        assert_eq!(merge_spec_filters(Some(vec![]), Some(vec![])), Some(vec![]));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd rust && cargo test -p pcbparts-smart-parser parser::`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

```rust
// rust/crates/pcbparts-smart-parser/src/parser.rs — insert above the tests module
use crate::connectors::{extract_connector_series, ConnectorSpec};
use crate::mapping::{infer_subcategory_from_values, map_value_to_spec};
use crate::models::extract_model_number;
use crate::packages::extract_package;
use crate::semantic::{connector_noise_words, extract_semantic_descriptors, remove_noise_words};
use crate::types::{extract_component_type, extract_mounting_type};
use crate::values::{extract_values, ExtractedValue};
use pcbparts_search::spec_filter::SpecFilter;
use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone)]
pub struct ParsedQuery {
    pub original: String,
    /// For FTS search.
    pub remaining_text: String,
    pub subcategory: Option<String>,
    pub spec_filters: Vec<SpecFilter>,
    pub package: Option<String>,
    pub model_number: Option<String>,
    /// "SMD" or "Through Hole".
    pub mounting_type: Option<String>,
    pub connector_spec: Option<ConnectorSpec>,
    pub detected: serde_json::Value,
}

static RADIAL_LEADED_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(radial|through.?hole|pth|leaded)\b").unwrap());
static TRIMMER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(trimmer|potentiometer|trimpot|variable\s*resistor)\b").unwrap());
static STANDALONE_NUMBER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(\d+)\b").unwrap());
static DUAL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bdual\b").unwrap());
static SINGLE_ROW_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(single|1)\s*row\b").unwrap());
static DOUBLE_ROW_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(double|dual|2)\s*row\b").unwrap());
static MAGNETICS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bmagnetics?\b").unwrap());
static SINGLE_LETTER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b[A-Za-z]\b").unwrap());
static ORPHANED_HYPHEN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*-\s*").unwrap());
static WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

const DIMENSION_AS_PACKAGE_CATEGORIES: [&str; 6] = [
    "inductors (smd)", "power inductors", "inductors, coils, chokes",
    "led", "leds", "light emitting diodes",
];
const CONNECTOR_WORDS: [&str; 6] = ["header", "connector", "terminal", "socket", "plug", "receptacle"];
const HEADER_KEYWORDS: [&str; 4] = ["header", "pin header", "male header", "female header"];

fn values_to_json(values: &[ExtractedValue]) -> serde_json::Value {
    serde_json::Value::Array(
        values
            .iter()
            .map(|v| serde_json::json!({"raw": v.raw, "type": v.unit_type, "normalized": v.normalized}))
            .collect(),
    )
}

/// Parse a natural language query into structured filters.
pub fn parse_smart_query(query: &str) -> ParsedQuery {
    let mut detected = serde_json::Map::new();
    let mut result = ParsedQuery {
        original: query.to_string(),
        remaining_text: query.to_string(),
        subcategory: None,
        spec_filters: Vec::new(),
        package: None,
        model_number: None,
        mounting_type: None,
        connector_spec: None,
        detected: serde_json::Value::Null,
    };
    let mut remaining = query.to_string();

    // Step 1: Extract model number (if present, it becomes the primary search term).
    let (model, after_model) = extract_model_number(&remaining);
    remaining = after_model;
    if let Some(ref m) = model {
        result.model_number = Some(m.clone());
        detected.insert("model_number".into(), serde_json::json!(m));
    }

    // Step 2: Extract package.
    let (package, after_pkg, pkg_suggested_subcat) = extract_package(&remaining);
    remaining = after_pkg;
    if let Some(ref p) = package {
        result.package = Some(p.clone());
        detected.insert("package".into(), serde_json::json!(p));
    }

    // Step 2b: Extract mounting type (PTH/THT -> Through Hole, SMD/SMT -> SMD).
    let (mounting_type, after_mount) = extract_mounting_type(&remaining);
    remaining = after_mount;
    if let Some(ref mt) = mounting_type {
        result.mounting_type = Some(mt.clone());
        detected.insert("mounting_type".into(), serde_json::json!(mt));
    }

    // Step 2c: Extract connector series and brand aliases BEFORE component type
    // extraction — keywords like "jst sh"/"qwiic" also appear in subcategory_aliases
    // and would otherwise be consumed as a generic "wire to board connector" with no
    // series info.
    let (connector_spec, after_conn) = extract_connector_series(&remaining);
    remaining = after_conn;
    if let Some(ref cs) = connector_spec {
        result.connector_spec = Some(cs.clone());
        result.subcategory = Some("wire to board connector".to_string());
        detected.insert(
            "connector_spec".into(),
            serde_json::json!({ "series": cs.series, "pitch": cs.pitch, "pins": cs.pins }),
        );
        detected.insert("subcategory".into(), serde_json::json!("wire to board connector"));
        // NOTE: pitch/pins are deliberately NOT added as spec filters here — most
        // connectors in the database have empty attributes dicts, so spec filters
        // would match nothing. Pitch/pin info lives in connector_spec and drives FTS
        // instead (Step 8b below). This mirrors commented-out code left in the
        // Python source for the same reason.
    }

    // Step 3: Extract component type (subcategory). Always runs, even after a
    // connector was already detected in Step 2c — `result.subcategory` can be
    // overwritten again here, matching Python exactly (no early-exit for connectors).
    let (subcategory, after_type, matched_keyword) = extract_component_type(&remaining);
    remaining = after_type;
    if let Some(ref subcat) = subcategory {
        result.subcategory = Some(subcat.clone());
        detected.insert("component_type".into(), serde_json::json!(matched_keyword));
        detected.insert("subcategory".into(), serde_json::json!(subcat));

        if let Some(ref kw) = matched_keyword {
            let kw_lower = kw.to_lowercase();
            if kw_lower.contains("n-channel") || kw_lower == "nmos" {
                result.spec_filters.push(SpecFilter::new("Type", "=", "N-Channel").expect("valid operator literal"));
            } else if kw_lower.contains("p-channel") || kw_lower == "pmos" {
                result.spec_filters.push(SpecFilter::new("Type", "=", "P-Channel").expect("valid operator literal"));
            } else if kw_lower == "npn" || kw_lower == "npn transistor" {
                result.spec_filters.push(SpecFilter::new("Type", "=", "NPN").expect("valid operator literal"));
            } else if kw_lower == "pnp" || kw_lower == "pnp transistor" {
                result.spec_filters.push(SpecFilter::new("Type", "=", "PNP").expect("valid operator literal"));
            }
        }

        // "radial"/"through hole" with electrolytic -> leaded capacitors.
        if subcat.to_lowercase() == "aluminum electrolytic capacitors - smd" && RADIAL_LEADED_RE.is_match(&remaining) {
            result.subcategory = Some("aluminum electrolytic capacitors - leaded".to_string());
            detected.insert("subcategory".into(), serde_json::json!("aluminum electrolytic capacitors - leaded"));
            remaining = RADIAL_LEADED_RE.replace_all(&remaining, "").trim().to_string();
            remaining = WHITESPACE_RE.replace_all(&remaining, " ").to_string();
        }
    } else if let Some(ref sc) = pkg_suggested_subcat {
        // Package-suggested subcategory (e.g. USB-C -> USB connectors).
        result.subcategory = Some(sc.clone());
        detected.insert("subcategory_from_package".into(), serde_json::json!(sc));
    }

    // Step 4: Extract numeric values.
    let (mut values, after_values) = extract_values(&remaining);
    remaining = after_values;
    if !values.is_empty() {
        detected.insert("values".into(), values_to_json(&values));
    }

    // Step 4a-pre: display resolutions like "128x64" look like dimensions but are not.
    if result.subcategory.as_deref().is_some_and(|s| s.to_lowercase().contains("display")) {
        values.retain(|v| v.unit_type != "dimensions");
        if !values.is_empty() {
            detected.insert("values".into(), values_to_json(&values));
        }
    }

    // Step 4a: standalone numbers as pin counts for connector types — handles "8 pin
    // header" where "pin header" was already extracted, leaving lone "8" behind.
    let is_connector = matched_keyword
        .as_ref()
        .is_some_and(|kw| CONNECTOR_WORDS.iter().any(|w| kw.to_lowercase().contains(w)));
    if is_connector {
        if let Some(m) = STANDALONE_NUMBER_RE.find(&remaining) {
            let num_val: i64 = m.as_str().parse().unwrap();
            if (1..=200).contains(&num_val) && !values.iter().any(|v| v.unit_type == "pin_count") {
                values.push(ExtractedValue {
                    raw: m.as_str().to_string(),
                    value: num_val as f64,
                    unit_type: "pin_count".to_string(),
                    normalized: format!("{num_val}P"),
                });
                detected
                    .entry("values")
                    .or_insert_with(|| serde_json::Value::Array(vec![]))
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"raw": m.as_str(), "type": "pin_count", "normalized": format!("{num_val}P")}));
                remaining = format!("{}{}", &remaining[..m.start()], &remaining[m.end()..]);
                remaining = WHITESPACE_RE.replace_all(remaining.trim(), " ").to_string();
            }
        }
    }

    // Step 4b: infer subcategory from values if not already set.
    if result.subcategory.is_none() && !values.is_empty() {
        if let Some(inferred) = infer_subcategory_from_values(&values) {
            detected.insert("subcategory_inferred".into(), serde_json::json!(inferred));
            result.subcategory = Some(inferred);
        }
    }

    // Step 4c: override subcategory for trimmer/potentiometer keywords — handles
    // "10K trimmer" where the value was detected before the keyword.
    if TRIMMER_RE.is_match(&remaining) {
        let overridable = result.subcategory.is_none()
            || result.subcategory.as_deref().map(str::to_lowercase).as_deref() == Some("chip resistor - surface mount");
        if overridable {
            result.subcategory = Some("potentiometers, variable resistors".to_string());
            detected.insert("subcategory".into(), serde_json::json!("potentiometers, variable resistors"));
            remaining = TRIMMER_RE.replace_all(&remaining, "").trim().to_string();
            remaining = WHITESPACE_RE.replace_all(&remaining, " ").to_string();
        }
    }

    // Step 4d: standalone numbers as impedance for ferrite beads — "ferrite bead 0603
    // 30" -> the "30" is parsed as 30Ω impedance.
    if result.subcategory.as_deref().map(str::to_lowercase).as_deref() == Some("ferrite beads") {
        if let Some(m) = STANDALONE_NUMBER_RE.find(&remaining) {
            let num_val: i64 = m.as_str().parse().unwrap();
            if (1..=5000).contains(&num_val) && !values.iter().any(|v| v.unit_type == "resistance") {
                values.push(ExtractedValue {
                    raw: m.as_str().to_string(),
                    value: num_val as f64,
                    unit_type: "resistance".to_string(),
                    normalized: format!("{num_val}Ohm"),
                });
                remaining = format!("{}{}", &remaining[..m.start()], &remaining[m.end()..]);
                remaining = WHITESPACE_RE.replace_all(remaining.trim(), " ").to_string();
            }
        }
    }

    // Step 5: extract semantic descriptors.
    let (semantic_filters, after_semantic) = extract_semantic_descriptors(&remaining);
    remaining = after_semantic;

    // Step 6: build spec filters from extracted values (category-aware).
    // `map_value_to_spec` and the connector text-cleanup checks below read the LOCAL
    // `subcategory` binding (its Step-3 snapshot) — NOT `result.subcategory`, which
    // Steps 4b/4c may have since reassigned. This distinction is load-bearing; see
    // Global Constraints.
    let subcat_lower = result.subcategory.clone().unwrap_or_default().to_lowercase();

    for value in &values {
        if value.unit_type == "dimensions" {
            let is_dim_as_package = DIMENSION_AS_PACKAGE_CATEGORIES.iter().any(|c| subcat_lower.contains(c));
            if is_dim_as_package {
                if result.package.is_none() {
                    result.package = Some(format!("SMD,{}", value.normalized));
                    detected.insert("package_from_dimensions".into(), serde_json::json!(result.package));
                }
                continue;
            }
        }

        // Most connectors have empty attributes dicts, so spec filters fail — pin
        // count/pitch/etc. drive FTS search instead (see Step 8b).
        if result.subcategory.as_deref().is_some_and(|s| s.to_lowercase().contains("connector")) {
            continue;
        }

        let (spec_name, operator) = map_value_to_spec(value, subcategory.as_deref(), matched_keyword.as_deref());
        result.spec_filters.push(SpecFilter::new(spec_name, operator, value.normalized.clone()).expect("valid operator literal"));
    }

    for sf in &semantic_filters {
        result.spec_filters.push(SpecFilter::new(sf.spec_name.clone(), sf.operator, sf.value.clone()).expect("valid operator literal"));
    }

    // Step 6b: "dual" for MOSFETs -> Number = "2 N-Channel"/"2 P-Channel". Reads
    // `result.subcategory` (current, post Steps 4b/4c), unlike Step 6 above.
    if result.subcategory.as_deref().map(str::to_lowercase).as_deref() == Some("mosfets") && DUAL_RE.is_match(&remaining) {
        let channel_type = result
            .spec_filters
            .iter()
            .find(|sf| sf.name == "Type" && (sf.value == "N-Channel" || sf.value == "P-Channel"))
            .map(|sf| sf.value.clone());
        if let Some(ct) = channel_type {
            result.spec_filters.push(SpecFilter::new("Number", "=", format!("2 {ct}")).expect("valid operator literal"));
        }
        remaining = DUAL_RE.replace_all(&remaining, "").trim().to_string();
        remaining = WHITESPACE_RE.replace_all(&remaining, " ").to_string();
    }

    // Step 6c: "single row"/"double row" for pin headers -> Pin Structure.
    let is_header = matched_keyword.as_ref().is_some_and(|kw| HEADER_KEYWORDS.iter().any(|h| kw.to_lowercase().contains(h)))
        || result.subcategory.as_deref().is_some_and(|s| s.to_lowercase().contains("header"));

    if is_header && SINGLE_ROW_RE.is_match(&remaining) {
        for sf in result.spec_filters.iter_mut() {
            if sf.name == "Number of Pins" && sf.value.ends_with('P') {
                let pin_count = &sf.value[..sf.value.len() - 1];
                if !pin_count.is_empty() && pin_count.chars().all(|c| c.is_ascii_digit()) {
                    *sf = SpecFilter::new("Pin Structure", "=", format!("1x{pin_count}P")).expect("valid operator literal");
                }
            }
        }
        remaining = SINGLE_ROW_RE.replace_all(&remaining, "").trim().to_string();
        remaining = WHITESPACE_RE.replace_all(&remaining, " ").to_string();
    }

    if is_header && DOUBLE_ROW_RE.is_match(&remaining) {
        for sf in result.spec_filters.iter_mut() {
            if sf.name == "Number of Pins" && sf.value.ends_with('P') {
                let pin_count_str = &sf.value[..sf.value.len() - 1];
                if let Ok(total) = pin_count_str.parse::<i64>() {
                    let pins_per_row = if total % 2 == 0 { total / 2 } else { total };
                    *sf = SpecFilter::new("Pin Structure", "=", format!("2x{pins_per_row}P")).expect("valid operator literal");
                }
            }
        }
        remaining = DOUBLE_ROW_RE.replace_all(&remaining, "").trim().to_string();
        remaining = WHITESPACE_RE.replace_all(&remaining, " ").to_string();
    }

    // Step 7: clean up remaining text. Step 7a/7b read the LOCAL `subcategory`
    // binding (Step-3 snapshot), like Step 6's map_value_to_spec call — not
    // `result.subcategory`.
    if subcategory.as_deref().is_some_and(|s| s.to_lowercase().contains("connector")) {
        // "magnetics" is common phrasing for RJ45-with-integrated-magnetics;
        // JLCPCB lists these as "Filtered" in descriptions.
        remaining = MAGNETICS_RE.replace_all(&remaining, "filtered").to_string();
    }

    remaining = remove_noise_words(&remaining);

    if subcategory.as_deref().is_some_and(|s| { let l = s.to_lowercase(); l.contains("connector") || l.contains("header") }) {
        let noise = connector_noise_words();
        remaining = remaining.split_whitespace().filter(|w| !noise.contains(w.to_lowercase().as_str())).collect::<Vec<_>>().join(" ");
    }

    remaining = SINGLE_LETTER_RE.replace_all(&remaining, "").to_string();
    remaining = ORPHANED_HYPHEN_RE.replace_all(&remaining, " ").to_string();
    remaining = WHITESPACE_RE.replace_all(remaining.trim(), " ").to_string();

    // Step 8: determine what to use for FTS search.
    if let Some(ref m) = model {
        result.remaining_text = m.clone();
    } else if !remaining.is_empty() && remaining.chars().count() >= 2 {
        result.remaining_text = remaining.clone();
    } else if !result.spec_filters.is_empty() || subcategory.is_some() {
        result.remaining_text = String::new();
    } else {
        result.remaining_text = query.to_string();
    }

    // Step 8b: add connector series term to FTS for better filtering.
    if let Some(ref cs) = result.connector_spec {
        if let Some(ref fts_term) = cs.fts_term {
            if !result.remaining_text.is_empty() {
                if !result.remaining_text.to_lowercase().contains(&fts_term.to_lowercase()) {
                    result.remaining_text = format!("{fts_term} {}", result.remaining_text);
                }
            } else {
                result.remaining_text = fts_term.clone();
            }
        }
    }

    result.detected = serde_json::Value::Object(detected);
    result
}

/// Merge manual and auto-detected spec filters. Manual filters take precedence for the
/// same attribute name (case-insensitive); auto-detected filters are added only if no
/// manual filter exists for that attribute. Returns `None` only if both inputs are
/// `None`/empty.
pub fn merge_spec_filters(
    manual_filters: Option<Vec<SpecFilter>>,
    auto_filters: Option<Vec<SpecFilter>>,
) -> Option<Vec<SpecFilter>> {
    let auto_filters = match auto_filters {
        Some(f) if !f.is_empty() => f,
        _ => return manual_filters,
    };
    let manual_filters = match manual_filters {
        Some(f) if !f.is_empty() => f,
        _ => return Some(auto_filters),
    };

    let manual_names: std::collections::HashSet<String> = manual_filters.iter().map(|f| f.name.to_lowercase()).collect();

    let mut merged = manual_filters;
    for auto_filter in auto_filters {
        if !manual_names.contains(&auto_filter.name.to_lowercase()) {
            merged.push(auto_filter);
        }
    }

    // `merged` is seeded from `manual_filters`, which is guaranteed non-empty at this
    // point (the second early-return above already handled the empty/None case) — so
    // this is always `Some`, matching Python's `merged if merged else None` (which is
    // likewise always truthy here).
    Some(merged)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd rust && cargo test -p pcbparts-smart-parser parser::`
Expected: PASS — 11/11 tests.

- [ ] **Step 5: Write `lib.rs`'s final module declarations and re-exports**

Mirrors `smart_parser/__init__.py`'s `__all__` — every public name a caller (Phase 5,
Phase 9) needs, re-exported at the crate root so `pcbparts_smart_parser::parse_smart_query`
works without reaching into `pcbparts_smart_parser::parser::parse_smart_query`. The raw
`PACKAGE_PATTERNS`/`MODEL_PATTERNS` regex lists that Python's `__all__` exposes are not
re-exported here — nothing outside `packages.rs`/`models.rs` consumes them directly in
either the Python codebase or this port; only the `extract_*` functions built from them
have real callers.

```rust
// rust/crates/pcbparts-smart-parser/src/lib.rs — final version
pub mod values;
pub mod models;
pub mod packages;
pub mod connectors;
pub mod semantic;
pub mod types;
pub mod mapping;
pub mod parser;

pub use connectors::{extract_connector_series, get_pitch_for_series, ConnectorSpec};
pub use mapping::{category_attribute_map, infer_subcategory_from_values, map_value_to_spec};
pub use models::extract_model_number;
pub use packages::extract_package;
pub use parser::{merge_spec_filters, parse_smart_query, ParsedQuery};
pub use semantic::{connector_noise_words, extract_semantic_descriptors, noise_words, remove_noise_words, SemanticFilter};
pub use types::{extract_component_type, extract_mounting_type};
pub use values::{extract_values, ExtractedValue};
```

- [ ] **Step 6: Run the full crate test suite to confirm nothing regressed**

Run: `cd rust && cargo test -p pcbparts-smart-parser`
Expected: PASS — 33/33 tests (13 in `values.rs`, 6 in `models.rs`+`packages.rs`, 4 in
`connectors.rs`, 3 in `semantic.rs`, 2 in `types.rs`, 2 in `mapping.rs`, 11 in
`parser.rs`) — then `cd rust && cargo test` to confirm the whole workspace (Phase 1
through Phase 4) still passes together with no cross-crate regressions.

- [ ] **Step 7: Commit**

```bash
git add rust/crates/pcbparts-smart-parser/src/parser.rs rust/crates/pcbparts-smart-parser/src/lib.rs
git commit -m "rust: port smart_parser/parser.py and __init__.py (crate re-exports)"
```
