# Rust Migration Phase 3: pcbparts-search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port `search/mpn.py`, `resolvers.py`, `spec_filter.py`, `query_builder.py`, `result.py`, and `engine.py` into a new `pcbparts-search` Rust crate, with `mpn.rs` ported 1:1 against its existing pytest suite and the other five files verified against newly generated characterization fixtures (golden values captured from the live Python code against a real `components.db`, since no pytest file exists for them today).

**Architecture:** One Rust module per Python file, same convention as Phase 2A/2B. `spec_filter.rs` and `query_builder.rs` depend on Phase 2A's `pcbparts-parsers` (`parsers::*`, `alternatives::{spec_parsers, dimension_spec_fields}`). `result.rs` depends on Phase 2A's `pcbparts-parsers::mounting::detect_mounting_type`. `engine.rs` depends on all of the above plus Phase 2A's `pcbparts-parsers::subcategory_aliases::{resolve_subcategory_name, find_similar_subcategories, SimilarSubcategory}` and uses `rusqlite::Connection` the same way Phase 1's `pcbparts-db` crate does (a borrowed `&Connection` passed into free functions/methods — see `rust/crates/pcbparts-db/src/boards/mod.rs` for the established pattern). `mpn.rs` and `resolvers.rs` have no in-crate dependencies.

**Tech Stack:** Rust 2021 edition, `rusqlite` (matching `pcbparts-db`'s version), `regex`, `serde_json` (with the `preserve_order` feature — required, see Global Constraints), `pcbparts-parsers` and `pcbparts-db` as path dependencies.

**Spec:** `docs/superpowers/specs/2026-08-22-rust-migration-design.md` (see the Phase 3 entry in "Migration Order" and its "corrected a third time" preamble)

## Global Constraints

- **Golden-value parity, two flavors.** `mpn.rs` is ported against `tests/test_resolvers.py`'s existing 15 pytest cases (real, pre-existing golden values — port 1:1). The other five files have **no existing pytest coverage** (confirmed in the spec's Phase 3 correction) — their tests assert against characterization fixtures captured in this plan from the live Python code running against a real, locally-built `data/components.db` (618,277 parts, 843 subcategories, 55 categories, as of this plan's writing). Every fixture value embedded in this plan's tasks was captured by actually running the Python functions — not hand-derived — so treat them the same as pre-existing pytest golden values: if a task's test fails against them, the port has a bug, not the fixture.
- **`data/components.db` must exist locally to regenerate or extend fixtures**, but the Rust crate itself never touches this file at build/test time (all fixture data is embedded as literal Rust values in each task's test code) — no test in this plan opens a real SQLite file. If a task's implementer needs to re-derive or double check a fixture, rebuild the db first: `docker run --rm -v "$(pwd)":/workspace -w /workspace python:3.12-slim python scripts/build_database.py --data-dir data --output data/components.db` (the script is stdlib-only plus this repo's own `pcbparts_mcp.parsers`, so no `pip install` is needed inside the container; takes about 30-45 seconds). `data/components.db` is git-ignored — never commit it.
- **`DEFAULT_MIN_STOCK` resolution:** Python's `search()` defaults `min_stock: int = DEFAULT_MIN_STOCK` (imported from `config.py`, Phase 9). Rust has no default arguments, so `SearchEngine::search()`'s `min_stock` parameter is a required `i64` — applying "10 if the caller didn't specify" is Phase 9's job when it wires the server together. No `config.py`/Phase 9 dependency in this crate.
- **`preserve_order` is required**, not optional. `result.rs`'s `row_to_dict` builds its `specs` map from an ordered array of `[name, value]` pairs (confirmed against the real `attributes` column: `[["Resistance", "10kΩ"], ["Operating Temperature", "-55℃~+155℃"], ...]`) — a Python dict comprehension over that array preserves insertion order. This is the same order-sensitivity Phase 2B found and fixed in `alternatives.rs`. `pcbparts-search/Cargo.toml` must declare `serde_json = { version = "1", features = ["preserve_order"] }` explicitly (Task 1) — do not rely on Cargo's workspace feature unification to supply it silently (that exact latent trap was found and rejected as a fix in Phase 2B's final review).
- **Determinism over `HashMap` iteration.** Any place this crate iterates a `HashMap`/`HashSet` to build an ordered result (not just doing point lookups) must sort for determinism, matching the precedent Phase 2A set (and had to fix once) in `subcategory_aliases::resolve_subcategory_name`. `SearchEngine::resolve_category_name` (Task 6) is the one new place this crate introduces such a scan — it's specified below with the sort already applied; don't regress it to an unsorted scan.
- **Never commit without explicit permission; no Claude attribution in any commit message** (this repo's CLAUDE.md, and established practice throughout this migration).
- **This plan lives on the `docs/rust-migration` branch, not `main`** — the repo owner is keeping planning docs off `main` for now. Do not commit this plan file (or the spec) to `main`; only the Rust crate code (`rust/crates/pcbparts-search/...`) is committed there, following the exact same commit-per-task pattern established in Phase 1/2A/2B.

## File Structure

```
rust/crates/pcbparts-search/
  Cargo.toml
  src/
    lib.rs               # pub mod declarations, one per module below
    mpn.rs               # normalize_mpn, looks_like_mpn
    resolvers.rs         # expand_query_synonyms, expand_package, resolve_manufacturer + data tables
    spec_filter.rs        # SpecFilter, SpecOperator, SPEC_TO_COLUMN, ATTRIBUTE_ALIASES, escape_like, generate_value_patterns, get_attribute_names
    query_builder.rs      # build_* clause functions, SqlParam, PostFilterMeta, GroupedFilter
    result.rs             # row_to_dict, SubcategoryInfo
    engine.rs             # SearchEngine, CategoryInfo
```

---

### Task 1: Crate scaffold + `mpn.rs`

**Files:**
- Create: `rust/crates/pcbparts-search/Cargo.toml`
- Create: `rust/crates/pcbparts-search/src/lib.rs`
- Create: `rust/crates/pcbparts-search/src/mpn.rs`
- Modify: `rust/Cargo.toml` (add `crates/pcbparts-search` to `members`)

**Interfaces:**
- Produces: `normalize_mpn(query: &str) -> Vec<String>`, `looks_like_mpn(query: &str) -> bool` — consumed by Task 6's `engine.rs` MPN-retry logic.

- [ ] **Step 1: Add the crate to the workspace**

```toml
# rust/Cargo.toml
[workspace]
resolver = "2"
members = ["crates/pcbparts-db", "crates/pcbparts-parsers", "crates/pcbparts-search"]
```

- [ ] **Step 2: Create the crate manifest**

Check `rust/crates/pcbparts-db/Cargo.toml` for the exact `rusqlite` version/features it declares and mirror it exactly (don't guess a version — copy it) — do the same for `pcbparts-parsers`' `regex` version. The manifest should look like:

```toml
# rust/crates/pcbparts-search/Cargo.toml
[package]
name = "pcbparts-search"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["preserve_order"] }
regex = "1"
rusqlite = { version = "<copy from pcbparts-db/Cargo.toml>", features = ["<copy from pcbparts-db/Cargo.toml>"] }
pcbparts-parsers = { path = "../pcbparts-parsers" }
pcbparts-db = { path = "../pcbparts-db" }
```

- [ ] **Step 3: Write `lib.rs`**

```rust
pub mod mpn;
```

(Only `mpn` for now — Tasks 2-6 each add their own `pub mod X;` line when they create that module, same incremental-declaration discipline Phase 2A used after its own Task 1 lib.rs mistake.)

- [ ] **Step 4: Write the failing tests for `mpn.rs`**

Ported 1:1 from `tests/test_resolvers.py` (all 15 cases — both `TestLooksLikeMpn` and `TestNormalizeMpn`).

```rust
// rust/crates/pcbparts-search/src/mpn.rs — tests module (write this first)
#[cfg(test)]
mod tests {
    use super::*;

    // --- TestLooksLikeMpn ---
    #[test]
    fn test_typical_ic_mpn() {
        assert!(looks_like_mpn("STM32F103C8T6"));
        assert!(looks_like_mpn("MCP73831-2ACI/MC"));
        assert!(looks_like_mpn("ESP32-C3"));
    }
    #[test]
    fn test_with_suffixes() {
        assert!(looks_like_mpn("STM32F103C8T6-TR"));
        assert!(looks_like_mpn("LM1117-3.3#PBF"));
    }
    #[test]
    fn test_short_mpn() {
        assert!(looks_like_mpn("NE555"));
        assert!(looks_like_mpn("1N4148"));
        assert!(looks_like_mpn("2N2222"));
    }
    #[test]
    fn test_not_mpn() {
        assert!(!looks_like_mpn("resistor"));
        assert!(!looks_like_mpn("10k"));
        assert!(!looks_like_mpn(""));
        assert!(!looks_like_mpn("abc"));
    }
    #[test]
    fn test_case_insensitive() {
        assert!(looks_like_mpn("stm32f103c8t6"));
        assert!(looks_like_mpn("Stm32F103c8T6"));
        assert!(looks_like_mpn("mcp73831-2aci/mc"));
    }

    // --- TestNormalizeMpn ---
    #[test]
    fn test_no_change_needed() {
        let result = normalize_mpn("LM1117-3.3");
        assert_eq!(result[0], "LM1117-3.3");
        assert_eq!(result.len(), 1);
    }
    #[test]
    fn test_strip_tr_suffix() {
        let result = normalize_mpn("STM32F103C8T6-TR");
        assert!(result.contains(&"STM32F103C8T6-TR".to_string()));
        assert!(result.contains(&"STM32F103C8T6".to_string()));
    }
    #[test]
    fn test_strip_pbf_suffix() {
        let result = normalize_mpn("LM1117-3.3#PBF");
        assert!(result.contains(&"LM1117-3.3#PBF".to_string()));
        assert!(result.contains(&"LM1117-3.3".to_string()));
    }
    #[test]
    fn test_insert_t_for_tape_reel() {
        let result = normalize_mpn("MCP73831-2ACI/MC");
        assert!(result.contains(&"MCP73831-2ACI/MC".to_string()));
        assert!(result.contains(&"MCP73831T-2ACI/MC".to_string()));
    }
    #[test]
    fn test_already_has_t() {
        let result = normalize_mpn("MCP73831T-2ACI/MC");
        assert!(!result.contains(&"MCP73831TT-2ACI/MC".to_string()));
    }
    #[test]
    fn test_original_always_first() {
        let result = normalize_mpn("MCP73831-2ACI/MC");
        assert_eq!(result[0], "MCP73831-2ACI/MC");
    }
    #[test]
    fn test_combined_strip_and_insert() {
        let result = normalize_mpn("MCP73831-2ACI-TR");
        assert!(result.contains(&"MCP73831-2ACI-TR".to_string()));
        assert!(result.contains(&"MCP73831-2ACI".to_string()));
        assert!(result.contains(&"MCP73831T-2ACI".to_string()));
    }
    #[test]
    fn test_lowercase_input() {
        let result = normalize_mpn("stm32f103c8t6-tr");
        assert_eq!(result[0], "stm32f103c8t6-tr");
        assert!(result.iter().any(|v| v.to_uppercase().contains("STM32F103C8T6")));
    }
    #[test]
    fn test_mixed_case_input() {
        let result = normalize_mpn("Stm32F103C8T6-TR");
        assert_eq!(result[0], "Stm32F103C8T6-TR");
        assert!(result.len() >= 2);
    }
    #[test]
    fn test_no_duplicate_variants() {
        let result = normalize_mpn("stm32f103c8t6-tr");
        let mut seen_upper = std::collections::HashSet::new();
        for v in &result {
            let v_upper = v.to_uppercase();
            assert!(!seen_upper.contains(&v_upper), "Duplicate variant: {v}");
            seen_upper.insert(v_upper);
        }
    }
}
```

- [ ] **Step 5: Run to verify failure**

Run: `cd rust && cargo test -p pcbparts-search`
Expected: FAIL to compile — `normalize_mpn`/`looks_like_mpn` don't exist yet.

- [ ] **Step 6: Write the implementation**

```rust
// rust/crates/pcbparts-search/src/mpn.rs — insert above the tests module
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

pub const MPN_TRAILING_SUFFIXES: &[&str] = &[
    "-TR", "/TR", "-T", "-CT", "-ND", "-DK", "#PBF", "-PBF", "#PBFREE", "-PBFREE", "+T", "+TR",
];

static MPN_INSERT_T_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^([A-Z]{2,5}\d{2,5})(-[A-Z0-9/]+)$").unwrap());

/// Generate normalized variants of an MPN query for better matching.
///
/// Returns variants in order of preference: original query first, then
/// trailing-suffix-stripped, then "T"-inserted (tape & reel) variants —
/// deduplicated case-insensitively.
pub fn normalize_mpn(query: &str) -> Vec<String> {
    let mut variants = vec![query.to_string()];
    let mut seen_upper: HashSet<String> = HashSet::new();
    seen_upper.insert(query.to_uppercase());
    let working = query.to_uppercase();

    let mut stripped = working.clone();
    for suffix in MPN_TRAILING_SUFFIXES {
        if stripped.ends_with(suffix) {
            stripped.truncate(stripped.len() - suffix.len());
            break;
        }
    }

    if !seen_upper.contains(&stripped.to_uppercase()) {
        variants.push(stripped.clone());
        seen_upper.insert(stripped.to_uppercase());
    }

    for candidate in [&working, &stripped] {
        if let Some(caps) = MPN_INSERT_T_PATTERN.captures(candidate) {
            let base = &caps[1];
            let suffix = &caps[2];
            if !base.ends_with('T') {
                let with_t = format!("{base}T{suffix}");
                if !seen_upper.contains(&with_t.to_uppercase()) {
                    seen_upper.insert(with_t.to_uppercase());
                    variants.push(with_t);
                }
            }
        }
    }

    variants
}

/// Check if a query looks like a manufacturer part number.
pub fn looks_like_mpn(query: &str) -> bool {
    let char_count = query.chars().count();
    if query.is_empty() || char_count < 4 || char_count > 40 {
        return false;
    }

    let has_letter = query.chars().any(|c| c.is_alphabetic());
    let has_digit = query.chars().any(|c| c.is_ascii_digit());
    if !(has_letter && has_digit) {
        return false;
    }

    static IC_STYLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^[A-Z]{1,5}\d{2,}").unwrap());
    if IC_STYLE.is_match(query) {
        return true;
    }

    static DIODE_STYLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\d[A-Z]\d{3,}").unwrap());
    if DIODE_STYLE.is_match(query) {
        return true;
    }

    query.contains('-') || query.contains('/')
}
```

Note: `normalize_mpn` collapses Python's two near-identical "try inserting T" blocks (one against `working`, one against `stripped`) into a single loop over `[&working, &stripped]` — same order, same behavior, less duplication. This is a deliberate, behavior-preserving simplification, not a deviation to flag as a concern.

- [ ] **Step 7: Run to verify pass**

Run: `cd rust && cargo test -p pcbparts-search`
Expected: PASS — 15/15 tests, pristine output.

- [ ] **Step 8: Commit**

```bash
git add rust/Cargo.toml rust/crates/pcbparts-search
git commit -m "rust: scaffold pcbparts-search crate, port mpn.py"
```

---

### Task 2: `resolvers.rs`

**Files:**
- Create: `rust/crates/pcbparts-search/src/resolvers.rs`
- Modify: `rust/crates/pcbparts-search/src/lib.rs` (add `pub mod resolvers;`)

**Interfaces:**
- Consumes: `pcbparts_parsers::manufacturer_aliases::{known_manufacturers, manufacturer_aliases}` (Phase 2A).
- Produces: `expand_query_synonyms(query: &str) -> String`, `expand_package(package: &str) -> Vec<String>`, `resolve_manufacturer(name: &str) -> String`, plus the data tables `package_families()`, `imperial_chip_sizes()`, `smd_package_families()` — consumed by Task 6's `engine.rs` (package expansion, manufacturer resolution) and Task 4's `query_builder.rs` is independent of this file.

**Characterization fixtures** (captured by running the live Python `search/resolvers.py` functions — golden values, not re-derived):

```json
{
  "expand_query_synonyms": {
    "U.FL connector": "IPEX connector",
    "i-pex 4pin": "IPEX 4pin",
    "MHF connector": "IPEX connector",
    "no match here": "no match here",
    "IPX": "IPEX"
  },
  "expand_package": {
    "SOT-23": ["SOT-23", "SOT-23-3", "SOT-23-3L", "SOT-23(TO-236)"],
    "0603": ["0603", "1608"],
    "3215": ["SMD3215", "SMD3215-2P", "SMD3215-4P", "SMD3215-8P"],
    "SMD3215": ["SMD3215", "SMD3215-2P", "SMD3215-4P", "SMD3215-8P"],
    "smd-3215-2p": ["SMD3215", "SMD3215-2P", "SMD3215-4P", "SMD3215-8P"],
    "QFN-24-EP(4x4)": ["QFN-24-EP(4x4)"],
    "unknown-pkg": ["unknown-pkg"]
  },
  "resolve_manufacturer": {
    "TI": "Texas Instruments",
    "texas instruments": "Texas Instruments",
    "YAGEO": "YAGEO",
    "yageo": "YAGEO",
    "Totally Unknown Co": "Totally Unknown Co"
  }
}
```

- [ ] **Step 1: Write the failing tests**

```rust
// rust/crates/pcbparts-search/src/resolvers.rs — tests module (write this first)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_query_synonyms() {
        assert_eq!(expand_query_synonyms("U.FL connector"), "IPEX connector");
        assert_eq!(expand_query_synonyms("i-pex 4pin"), "IPEX 4pin");
        assert_eq!(expand_query_synonyms("MHF connector"), "IPEX connector");
        assert_eq!(expand_query_synonyms("no match here"), "no match here");
        assert_eq!(expand_query_synonyms("IPX"), "IPEX");
    }

    #[test]
    fn test_expand_package_family() {
        assert_eq!(
            expand_package("SOT-23"),
            vec!["SOT-23", "SOT-23-3", "SOT-23-3L", "SOT-23(TO-236)"]
        );
        assert_eq!(expand_package("0603"), vec!["0603", "1608"]);
    }

    #[test]
    fn test_expand_package_smd_bare_dimension() {
        assert_eq!(
            expand_package("3215"),
            vec!["SMD3215", "SMD3215-2P", "SMD3215-4P", "SMD3215-8P"]
        );
    }

    #[test]
    fn test_expand_package_smd_prefix() {
        assert_eq!(
            expand_package("SMD3215"),
            vec!["SMD3215", "SMD3215-2P", "SMD3215-4P", "SMD3215-8P"]
        );
        assert_eq!(
            expand_package("smd-3215-2p"),
            vec!["SMD3215", "SMD3215-2P", "SMD3215-4P", "SMD3215-8P"]
        );
    }

    #[test]
    fn test_expand_package_no_expansion() {
        assert_eq!(expand_package("QFN-24-EP(4x4)"), vec!["QFN-24-EP(4x4)"]);
        assert_eq!(expand_package("unknown-pkg"), vec!["unknown-pkg"]);
    }

    #[test]
    fn test_resolve_manufacturer_alias() {
        assert_eq!(resolve_manufacturer("TI"), "Texas Instruments");
        assert_eq!(resolve_manufacturer("texas instruments"), "Texas Instruments");
    }

    #[test]
    fn test_resolve_manufacturer_known_case_insensitive() {
        assert_eq!(resolve_manufacturer("YAGEO"), "YAGEO");
        assert_eq!(resolve_manufacturer("yageo"), "YAGEO");
    }

    #[test]
    fn test_resolve_manufacturer_unknown_passthrough() {
        assert_eq!(resolve_manufacturer("Totally Unknown Co"), "Totally Unknown Co");
    }

    #[test]
    fn test_package_families_count() {
        // 4 imperial + 3 sot23-variants + 1 sot223 + 1 sot89 + 3 to-packages
        // + 3 qfn + 9 so/sop/soic aliases = 24 keys total
        assert_eq!(package_families().len(), 24);
    }

    #[test]
    fn test_imperial_chip_sizes_excludes_from_smd_expansion() {
        // "0402" is an imperial chip size, so even though it's a bare 4-digit
        // string it must NOT be looked up in SMD_PACKAGE_FAMILIES.
        assert!(imperial_chip_sizes().contains("0402"));
        assert_eq!(expand_package("0402"), vec!["0402", "1005"]); // handled by PACKAGE_FAMILIES instead
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd rust && cargo test -p pcbparts-search resolvers::`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

```rust
// rust/crates/pcbparts-search/src/resolvers.rs — insert above the tests module
use pcbparts_parsers::manufacturer_aliases::{known_manufacturers, manufacturer_aliases};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

struct SynonymGroup {
    primary: &'static str,
    patterns: Vec<Regex>,
}

static SYNONYM_GROUPS: LazyLock<Vec<SynonymGroup>> = LazyLock::new(|| {
    vec![SynonymGroup {
        primary: "IPEX",
        patterns: vec![
            Regex::new(r"(?i)u\.fl").unwrap(),
            Regex::new(r"(?i)mhf").unwrap(),
            Regex::new(r"(?i)i-pex").unwrap(),
            Regex::new(r"(?i)hirose u\.fl").unwrap(),
            Regex::new(r"(?i)ipx").unwrap(),
        ],
    }]
});

/// Expand query with synonyms for better search results (e.g. "U.FL" -> "IPEX").
pub fn expand_query_synonyms(query: &str) -> String {
    let mut result = query.to_string();
    for group in SYNONYM_GROUPS.iter() {
        for pattern in &group.patterns {
            if pattern.is_match(&result) {
                result = pattern.replace_all(&result, group.primary).to_string();
                break;
            }
        }
    }
    result
}

pub fn package_families() -> HashMap<&'static str, Vec<&'static str>> {
    HashMap::from([
        ("0402", vec!["0402", "1005"]),
        ("0603", vec!["0603", "1608"]),
        ("0805", vec!["0805", "2012"]),
        ("1206", vec!["1206", "3216"]),
        ("sot-23", vec!["SOT-23", "SOT-23-3", "SOT-23-3L", "SOT-23(TO-236)"]),
        ("sot-23-5", vec!["SOT-23-5", "SOT-23-5L"]),
        ("sot-23-6", vec!["SOT-23-6", "SOT-23-6L"]),
        ("sot-223", vec!["SOT-223", "SOT-223-3", "SOT-223-3L", "SOT-223-4"]),
        ("sot-89", vec!["SOT-89", "SOT-89-3", "SOT-89-3L"]),
        ("to-252", vec!["TO-252", "TO-252-2", "TO-252-2L", "DPAK"]),
        ("to-263", vec!["TO-263", "TO-263-2", "D2PAK"]),
        ("to-220", vec!["TO-220", "TO-220-3", "TO-220F", "TO-220F-3"]),
        ("qfn-16", vec!["QFN-16", "QFN-16-EP(3x3)", "QFN-16-EP(4x4)", "QFN-16(3x3)", "VQFN-16"]),
        ("qfn-24", vec!["QFN-24", "QFN-24-EP(4x4)", "VQFN-24", "VQFN-24-EP(4x4)"]),
        ("qfn-32", vec!["QFN-32", "QFN-32-EP(5x5)", "VQFN-32", "VQFN-32-EP(5x5)"]),
        ("so-8", vec!["SO-8", "SOP-8", "SOIC-8"]),
        ("sop-8", vec!["SO-8", "SOP-8", "SOIC-8"]),
        ("soic-8", vec!["SO-8", "SOP-8", "SOIC-8"]),
        ("so8", vec!["SO-8", "SOP-8", "SOIC-8"]),
        ("sop8", vec!["SO-8", "SOP-8", "SOIC-8"]),
        ("soic8", vec!["SO-8", "SOP-8", "SOIC-8"]),
        ("so-16", vec!["SO-16", "SOP-16", "SOIC-16"]),
        ("sop-16", vec!["SO-16", "SOP-16", "SOIC-16"]),
        ("soic-16", vec!["SO-16", "SOP-16", "SOIC-16"]),
    ])
}

pub fn imperial_chip_sizes() -> HashSet<&'static str> {
    HashSet::from([
        "01005", "0201", "03015", "0402", "0603", "0612", "0805", "0806",
        "1008", "1206", "1210", "1212", "1218", "1806", "1808", "1812",
        "2010", "2220", "2410", "2512", "2920", "3920", "5930",
    ])
}

pub fn smd_package_families() -> HashMap<&'static str, Vec<&'static str>> {
    HashMap::from([
        ("1610", vec!["SMD1610", "SMD1610-2P"]),
        ("1612", vec!["SMD1612-4P"]),
        ("2012", vec!["SMD2012-2P", "SMD2012-4P", "SMD2012-8P"]),
        ("2016", vec!["SMD2016", "SMD2016-2P", "SMD2016-4P", "SMD2016-6P"]),
        ("2520", vec!["SMD2520", "SMD2520-2P", "SMD2520-4P", "SMD2520-6P"]),
        ("2835", vec!["SMD2835", "SMD2835-2P", "SMD2835-3P", "SMD2835-4P", "SMD2835-6P"]),
        ("3014", vec!["SMD3014-2P"]),
        ("3020", vec!["SMD3020", "SMD3020-3P"]),
        ("3030", vec!["SMD3030", "SMD3030-2P", "SMD3030-3P", "SMD3030-4P", "SMD3030-6P", "SMD3030-7P"]),
        ("3215", vec!["SMD3215", "SMD3215-2P", "SMD3215-4P", "SMD3215-8P"]),
        ("3225", vec!["SMD3225", "SMD3225-2P", "SMD3225-4P", "SMD3225-6P", "SMD3225-10P", "SMD3225-14P", "SMD-3225_4P"]),
        ("3528", vec!["SMD3528", "SMD3528-2P", "SMD3528-3P", "SMD3528-4P", "SMD3528-6P"]),
        ("3535", vec!["SMD3535", "SMD3535-2P", "SMD3535-3P", "SMD3535-4P", "SMD3535-5P", "SMD3535-6P"]),
        ("5032", vec!["SMD5032", "SMD5032-2P", "SMD5032-4P", "SMD5032-6P", "SMD-5032-4P"]),
        ("5050", vec!["SMD5050", "SMD5050-2P", "SMD5050-4P", "SMD5050-6P", "SMD5050-8P"]),
        ("5730", vec!["SMD5730", "SMD5730-3P"]),
        ("6035", vec!["SMD6035-2P", "SMD6035-4P"]),
        ("7050", vec!["SMD7050", "SMD7050-2P", "SMD7050-4P", "SMD7050-6P", "SMD7050-10P"]),
        ("7060", vec!["SMD7060", "SMD7060-2P", "SMD7060-3P"]),
        ("8045", vec!["SMD8045-2P"]),
        ("8080", vec!["SMD8080-2P", "SMD8080-3P", "SMD8080-4P", "SMD8080-5P", "SMD8080-6P"]),
        ("9070", vec!["SMD9070-8P"]),
    ])
}

static BARE_DIMENSION_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{4}$").unwrap());
static SMD_PREFIX_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^smd-?(\d{4,5})(?:-\d+p)?$").unwrap());

/// Expand a package name to include family variants.
pub fn expand_package(package: &str) -> Vec<String> {
    let pkg_lower = package.to_lowercase();

    if let Some(variants) = package_families().get(pkg_lower.as_str()) {
        return variants.iter().map(|s| s.to_string()).collect();
    }

    if BARE_DIMENSION_RE.is_match(package) && !imperial_chip_sizes().contains(package) {
        if let Some(variants) = smd_package_families().get(package) {
            return variants.iter().map(|s| s.to_string()).collect();
        }
    }

    if let Some(caps) = SMD_PREFIX_RE.captures(&pkg_lower) {
        let dim = &caps[1];
        if let Some(variants) = smd_package_families().get(dim) {
            return variants.iter().map(|s| s.to_string()).collect();
        }
    }

    vec![package.to_string()]
}

fn manufacturer_lower_to_exact() -> HashMap<String, &'static str> {
    known_manufacturers().into_iter().map(|name| (name.to_lowercase(), name)).collect()
}

/// Resolve manufacturer alias to canonical name.
pub fn resolve_manufacturer(name: &str) -> String {
    let name_lower = name.to_lowercase();

    if let Some(&canonical) = manufacturer_aliases().get(name_lower.as_str()) {
        return canonical.to_string();
    }

    if let Some(&exact) = manufacturer_lower_to_exact().get(&name_lower) {
        return exact.to_string();
    }

    name.to_string()
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cd rust && cargo test -p pcbparts-search resolvers::`
Expected: PASS — 9/9 tests, pristine output.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/pcbparts-search/src/resolvers.rs rust/crates/pcbparts-search/src/lib.rs
git commit -m "rust: port search/resolvers.py (characterization tests, no prior pytest coverage)"
```

---

### Task 3: `spec_filter.rs`

**Files:**
- Create: `rust/crates/pcbparts-search/src/spec_filter.rs`
- Modify: `rust/crates/pcbparts-search/src/lib.rs` (add `pub mod spec_filter;`)

**Interfaces:**
- Consumes: `pcbparts_parsers::parsers::*` (Phase 2A parse functions), `pcbparts_parsers::alternatives::spec_parsers` (Phase 2B).
- Produces: `SpecOperator` enum, `SpecFilter` struct, `SpecParserFn` type alias (`fn(&str) -> Option<f64>`), `spec_to_column() -> HashMap<&'static str, (&'static str, Option<SpecParserFn>)>`, `attribute_aliases() -> HashMap<&'static str, Vec<&'static str>>`, `escape_like`, `generate_value_patterns`, `get_attribute_names` — consumed by Task 4's `query_builder.rs` and Task 6's `engine.rs`.

**Deliberate deviation from Python:** Python's `SpecFilter.__post_init__` raises `ValueError("SpecFilter name and value must be strings")` if `name`/`value` aren't strings. Rust's type system makes this unreachable at compile time (`name: String, value: String` in the struct — there is no way to construct one with a non-string field), so this check is not ported; only the operator-validity check (which IS a real runtime possibility, since the operator string comes from user input) is ported, as `SpecFilter::new`'s `Result` error.

**Characterization fixtures** (captured by running the live Python `search/spec_filter.py`):

```json
{
  "spec_filter_valid": {"name": "Capacitance", "op": ">=", "value": "10uF"},
  "spec_filter_invalid_operator_error": "Invalid operator '!='. Must be one of: <, <=, =, >, >=",
  "escape_like": {"50%": "50\\%", "a_b": "a\\_b", "back\\slash": "back\\\\slash", "plain": "plain"},
  "generate_value_patterns": {
    "Resistance|82k": ["%\"Resistance\", \"82k%", "%\"Resistance\", \"82K%", "%\"Resistance\", \"82k%"],
    "Capacitance|10uF": ["%\"Capacitance\", \"10uF%", "%\"Capacitance\", \"10uf%", "%\"Capacitance\", \"10u%"],
    "Tolerance|5%": ["%\"Tolerance\", \"5\\%%", "%\"Tolerance\", \"\\\\u00b15\\%%"],
    "Unknown|x_none_parsed": []
  },
  "get_attribute_names": {
    "Vgs(th)": ["Gate Threshold Voltage (Vgs(th))", "Gate Threshold Voltage"],
    "Resistance": ["Resistance"],
    "Drain to Source Voltage": ["Drain to Source Voltage"],
    "Totally Unknown Spec": ["Totally Unknown Spec"]
  },
  "SPEC_TO_COLUMN_count": 54,
  "ATTRIBUTE_ALIASES_count": 20
}
```

Note on `generate_value_patterns`'s literal `\\u00b1` in the Tolerance pattern: this is Python's actual, verified output — the source string `f'...\\\\u00b1{...}\\%%'` produces the literal four characters `\`, `\`, `u`, `0`... (i.e. two literal backslashes followed by the text `u00b1`), NOT the `±` Unicode character. This looks like it could be a latent bug in the Python source (a single `\\` — one backslash — would have produced the actual `±` character via a unicode escape), but it is verified, real, current behavior — port it exactly as observed. Do not "fix" it.

- [ ] **Step 1: Write the failing tests**

```rust
// rust/crates/pcbparts-search/src/spec_filter.rs — tests module (write this first)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_filter_valid() {
        let sf = SpecFilter::new("Capacitance", ">=", "10uF").unwrap();
        assert_eq!(sf.to_dict()["name"], "Capacitance");
        assert_eq!(sf.to_dict()["op"], ">=");
        assert_eq!(sf.to_dict()["value"], "10uF");
    }

    #[test]
    fn test_spec_filter_invalid_operator() {
        let err = SpecFilter::new("X", "!=", "1").unwrap_err();
        assert_eq!(err, "Invalid operator '!='. Must be one of: <, <=, =, >, >=");
    }

    #[test]
    fn test_escape_like() {
        assert_eq!(escape_like("50%"), "50\\%");
        assert_eq!(escape_like("a_b"), "a\\_b");
        assert_eq!(escape_like("back\\slash"), "back\\\\slash");
        assert_eq!(escape_like("plain"), "plain");
    }

    #[test]
    fn test_generate_value_patterns_resistance() {
        let patterns = generate_value_patterns("Resistance", "82k", Some(82000.0));
        assert_eq!(
            patterns,
            vec!["%\"Resistance\", \"82k%", "%\"Resistance\", \"82K%", "%\"Resistance\", \"82k%"]
        );
    }

    #[test]
    fn test_generate_value_patterns_capacitance() {
        let patterns = generate_value_patterns("Capacitance", "10uF", Some(10e-6));
        assert_eq!(
            patterns,
            vec!["%\"Capacitance\", \"10uF%", "%\"Capacitance\", \"10uf%", "%\"Capacitance\", \"10u%"]
        );
    }

    #[test]
    fn test_generate_value_patterns_tolerance() {
        let patterns = generate_value_patterns("Tolerance", "5%", Some(5.0));
        assert_eq!(
            patterns,
            vec!["%\"Tolerance\", \"5\\%%", "%\"Tolerance\", \"\\\\u00b15\\%%"]
        );
    }

    #[test]
    fn test_generate_value_patterns_none_parsed() {
        assert_eq!(generate_value_patterns("Unknown", "x", None), Vec::<String>::new());
    }

    #[test]
    fn test_get_attribute_names_alias() {
        assert_eq!(
            get_attribute_names("Vgs(th)"),
            vec!["Gate Threshold Voltage (Vgs(th))", "Gate Threshold Voltage"]
        );
    }

    #[test]
    fn test_get_attribute_names_full_name_with_parser() {
        assert_eq!(get_attribute_names("Resistance"), vec!["Resistance"]);
    }

    #[test]
    fn test_get_attribute_names_reverse_lookup() {
        assert_eq!(get_attribute_names("Drain to Source Voltage"), vec!["Drain to Source Voltage"]);
    }

    #[test]
    fn test_get_attribute_names_unknown_passthrough() {
        assert_eq!(get_attribute_names("Totally Unknown Spec"), vec!["Totally Unknown Spec"]);
    }

    #[test]
    fn test_spec_to_column_count() {
        assert_eq!(spec_to_column().len(), 54);
    }

    #[test]
    fn test_attribute_aliases_count() {
        assert_eq!(attribute_aliases().len(), 20);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd rust && cargo test -p pcbparts-search spec_filter::`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

```rust
// rust/crates/pcbparts-search/src/spec_filter.rs — insert above the tests module
use pcbparts_parsers::alternatives::{spec_parsers, SpecParser};
use pcbparts_parsers::parsers::*;
use std::collections::HashMap;

pub type SpecParserFn = fn(&str) -> Option<f64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecOperator {
    Eq,
    Ge,
    Le,
    Gt,
    Lt,
}

impl SpecOperator {
    pub fn as_str(&self) -> &'static str {
        match self {
            SpecOperator::Eq => "=",
            SpecOperator::Ge => ">=",
            SpecOperator::Le => "<=",
            SpecOperator::Gt => ">",
            SpecOperator::Lt => "<",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "=" => Ok(SpecOperator::Eq),
            ">=" => Ok(SpecOperator::Ge),
            "<=" => Ok(SpecOperator::Le),
            ">" => Ok(SpecOperator::Gt),
            "<" => Ok(SpecOperator::Lt),
            other => Err(format!("Invalid operator '{other}'. Must be one of: <, <=, =, >, >=")),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpecFilter {
    pub name: String,
    pub operator: SpecOperator,
    pub value: String,
}

impl SpecFilter {
    pub fn new(name: impl Into<String>, operator: &str, value: impl Into<String>) -> Result<Self, String> {
        let operator = SpecOperator::parse(operator)?;
        Ok(Self { name: name.into(), operator, value: value.into() })
    }

    pub fn to_dict(&self) -> serde_json::Value {
        serde_json::json!({"name": self.name, "op": self.operator.as_str(), "value": self.value})
    }
}

pub fn spec_to_column() -> HashMap<&'static str, (&'static str, Option<SpecParserFn>)> {
    HashMap::from([
        ("Resistance", ("resistance_ohms", Some(parse_resistance as SpecParserFn))),
        ("Capacitance", ("capacitance_f", Some(parse_capacitance as SpecParserFn))),
        ("Inductance", ("inductance_h", Some(parse_inductance as SpecParserFn))),
        ("DC Resistance(DCR)", ("dcr_ohms", Some(parse_resistance as SpecParserFn))),
        ("DCR", ("dcr_ohms", Some(parse_resistance as SpecParserFn))),
        ("Current - Saturation(Isat)", ("isat_a", Some(parse_current as SpecParserFn))),
        ("Current - Saturation (Isat)", ("isat_a", Some(parse_current as SpecParserFn))),
        ("Isat", ("isat_a", Some(parse_current as SpecParserFn))),
        ("Voltage Rating", ("voltage_max_v", Some(parse_voltage as SpecParserFn))),
        ("Voltage", ("voltage_max_v", Some(parse_voltage as SpecParserFn))),
        ("Current Rating", ("current_max_a", Some(parse_current as SpecParserFn))),
        ("Tolerance", ("tolerance_pct", Some(parse_tolerance as SpecParserFn))),
        ("Power(Watts)", ("power_w", Some(parse_power as SpecParserFn))),
        ("Power", ("power_w", Some(parse_power as SpecParserFn))),
        ("Pd - Power Dissipation", ("power_w", Some(parse_power as SpecParserFn))),
        ("Drain to Source Voltage", ("vds_max_v", Some(parse_voltage as SpecParserFn))),
        ("Vds", ("vds_max_v", Some(parse_voltage as SpecParserFn))),
        ("Current - Continuous Drain(Id)", ("id_max_a", Some(parse_current as SpecParserFn))),
        ("Id", ("id_max_a", Some(parse_current as SpecParserFn))),
        ("RDS(on)", ("rds_on_ohms", Some(parse_resistance as SpecParserFn))),
        ("Rds(on)", ("rds_on_ohms", Some(parse_resistance as SpecParserFn))),
        ("Voltage - DC Reverse(Vr)", ("vr_max_v", Some(parse_voltage as SpecParserFn))),
        ("Vr", ("vr_max_v", Some(parse_voltage as SpecParserFn))),
        ("Current - Rectified", ("if_max_a", Some(parse_current as SpecParserFn))),
        ("If", ("if_max_a", Some(parse_current as SpecParserFn))),
        ("Voltage - Forward(Vf@If)", ("vf_v", Some(parse_voltage as SpecParserFn))),
        ("Vf", ("vf_v", Some(parse_voltage as SpecParserFn))),
        ("Output Voltage", ("vout_v", Some(parse_voltage as SpecParserFn))),
        ("Vout", ("vout_v", Some(parse_voltage as SpecParserFn))),
        ("Output Current", ("iout_max_a", Some(parse_current as SpecParserFn))),
        ("Iout", ("iout_max_a", Some(parse_current as SpecParserFn))),
        ("Voltage Dropout", ("vdropout_v", Some(parse_voltage as SpecParserFn))),
        ("Quiescent Current(Iq)", ("iq_ua", Some(parse_current as SpecParserFn))),
        ("Quiescent Current", ("iq_ua", Some(parse_current as SpecParserFn))),
        ("Sampling Rate", ("sample_rate_hz", Some(parse_frequency as SpecParserFn))),
        ("Load Capacitance", ("load_capacitance_pf", Some(parse_capacitance as SpecParserFn))),
        ("Frequency Stability", ("freq_tolerance_ppm", Some(parse_ppm as SpecParserFn))),
        ("Gain Bandwidth Product", ("gbw_hz", Some(parse_frequency as SpecParserFn))),
        ("Ripple Current", ("ripple_current_a", Some(parse_current as SpecParserFn))),
        ("Equivalent Series Resistance(ESR)", ("esr_ohms", Some(parse_resistance as SpecParserFn))),
        ("ESR", ("esr_ohms", Some(parse_resistance as SpecParserFn))),
        ("Flash", ("flash_size_bytes", None)),
        ("Program Memory Size", ("flash_size_bytes", None)),
        ("SRAM", ("ram_size_bytes", None)),
        ("RAM Size", ("ram_size_bytes", None)),
        ("Speed", ("clock_speed_hz", Some(parse_frequency as SpecParserFn))),
        ("CPU Maximum Speed", ("clock_speed_hz", Some(parse_frequency as SpecParserFn))),
        ("Capacity", ("memory_capacity_bits", None)),
        ("Memory Size", ("memory_capacity_bits", None)),
        ("Charging Current", ("charge_current_a", Some(parse_current as SpecParserFn))),
        ("Charge Current - Max", ("charge_current_a", Some(parse_current as SpecParserFn))),
        ("Clamping Voltage", ("clamping_voltage_v", Some(parse_voltage as SpecParserFn))),
        ("Reverse Stand-Off Voltage (Vrwm)", ("standoff_voltage_v", Some(parse_voltage as SpecParserFn))),
        ("Peak Pulse Power(Ppk)", ("surge_power_w", Some(parse_power as SpecParserFn))),
    ])
}

pub fn attribute_aliases() -> HashMap<&'static str, Vec<&'static str>> {
    HashMap::from([
        ("Vgs(th)", vec!["Gate Threshold Voltage (Vgs(th))", "Gate Threshold Voltage"]),
        ("Vds", vec!["Drain to Source Voltage"]),
        ("Id", vec!["Current - Continuous Drain(Id)"]),
        ("Rds(on)", vec!["RDS(on)"]),
        ("Vr", vec!["Voltage - DC Reverse(Vr)"]),
        ("If", vec!["Current - Rectified"]),
        ("Vf", vec!["Voltage - Forward(Vf@If)"]),
        ("Capacitance", vec!["Capacitance"]),
        ("Voltage", vec!["Voltage Rating"]),
        ("Tolerance", vec!["Tolerance"]),
        ("Power", vec!["Power(Watts)", "Pd - Power Dissipation"]),
        ("Resistance", vec!["Resistance"]),
        ("Inductance", vec!["Inductance"]),
        ("DCR", vec!["DC Resistance(DCR)"]),
        ("Isat", vec!["Current - Saturation(Isat)", "Current - Saturation (Isat)"]),
        ("Frequency", vec!["Frequency"]),
        ("Vceo", vec!["Collector - Emitter Voltage VCEO"]),
        ("Ic", vec!["Current - Collector(Ic)"]),
        ("Vout", vec!["Output Voltage"]),
        ("Iout", vec!["Output Current"]),
    ])
}

fn attr_full_to_aliases() -> HashMap<&'static str, Vec<&'static str>> {
    let mut map: HashMap<&'static str, Vec<&'static str>> = HashMap::new();
    for (alias, full_names) in attribute_aliases() {
        for full_name in full_names {
            map.entry(full_name).or_default().push(alias);
        }
    }
    map
}

/// Escape SQL LIKE wildcards (%, _) using backslash as the escape character.
pub fn escape_like(value: &str) -> String {
    value.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

fn is_integer(value: f64, tol: f64) -> bool {
    (value - value.round()).abs() < tol
}

/// Generate SQL LIKE patterns that match the actual spec value in JSON.
pub fn generate_value_patterns(spec_name: &str, value: &str, parsed_value: Option<f64>) -> Vec<String> {
    let Some(parsed_value) = parsed_value else { return vec![] };

    let name_escaped = escape_like(spec_name);
    // Python: value.rstrip("OhmOHMohm") strips any trailing chars in the SET
    // {O,h,m,H,M,o}, not the literal substring "OhmOHMohm".
    let value_stripped = value.trim_end_matches(['O', 'h', 'm', 'H', 'M', 'o']);
    let value_escaped = escape_like(value_stripped);

    let mut patterns = vec![format!("%\"{name_escaped}\", \"{value_escaped}%")];

    let value_lower = value_escaped.to_lowercase();
    let value_upper = value_escaped.to_uppercase();
    if value_lower != value_upper {
        if value_escaped == value_lower {
            patterns.push(format!("%\"{name_escaped}\", \"{value_upper}%"));
        } else {
            patterns.push(format!("%\"{name_escaped}\", \"{value_lower}%"));
        }
    }

    let spec_name_lower = spec_name.to_lowercase();
    if spec_name_lower.contains("resistance") && parsed_value >= 1000.0 {
        let k_val = parsed_value / 1000.0;
        if is_integer(k_val, 1e-9) {
            patterns.push(format!("%\"{name_escaped}\", \"{}k%", k_val.round() as i64));
        }
    } else if spec_name_lower.contains("capacitance") {
        let uf = parsed_value * 1e6;
        if uf >= 1.0 && is_integer(uf, 1e-9) {
            patterns.push(format!("%\"{name_escaped}\", \"{}u%", uf.round() as i64));
        }
    } else if spec_name_lower.contains("tolerance") && is_integer(parsed_value, 1e-9) {
        patterns.push(format!("%\"{name_escaped}\", \"\\\\u00b1{}\\%%", parsed_value.round() as i64));
    }

    patterns.truncate(3);
    patterns
}

/// Get all possible attribute names for a given name (including aliases).
pub fn get_attribute_names(name: &str) -> Vec<String> {
    let aliases = attribute_aliases();
    if let Some(full_names) = aliases.get(name) {
        return full_names.iter().map(|s| s.to_string()).collect();
    }
    if matches!(spec_parsers().get(name), Some(SpecParser::Parser(_)) | Some(SpecParser::Special) | Some(SpecParser::StringMatch)) {
        return vec![name.to_string()];
    }
    let full_to_aliases = attr_full_to_aliases();
    if let Some(alias_list) = full_to_aliases.get(name) {
        let first_alias = alias_list[0];
        if let Some(full_names) = aliases.get(first_alias) {
            return full_names.iter().map(|s| s.to_string()).collect();
        }
    }
    vec![name.to_string()]
}
```

Note: `if name in SPEC_PARSERS:` in Python is a plain key-membership check (any value, including `None`/`"special"`, counts) — so the Rust equivalent is `spec_parsers().contains_key(name)`, not a match on the variant. Use `pcbparts_parsers::alternatives::spec_parsers().contains_key(name)` directly rather than the `matches!` shown above (simplify before committing — the `matches!` form was overcomplicated during drafting; a plain `.contains_key(name)` is correct and simpler).

Note on `value.rstrip("OhmOHMohm")`: Python's `str.rstrip(chars)` treats its argument as a *set* of characters to strip, not a substring — the set here is `{O, h, m, O, H, M, o, h, m}` which deduplicates to `{O, h, m, H, M, o}` (6 distinct characters). The `trim_end_matches([...])` call above lists all 6.

- [ ] **Step 4: Run to verify pass**

Run: `cd rust && cargo test -p pcbparts-search spec_filter::`
Expected: PASS — 13/13 tests, pristine output.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/pcbparts-search/src/spec_filter.rs rust/crates/pcbparts-search/src/lib.rs
git commit -m "rust: port search/spec_filter.py (characterization tests, no prior pytest coverage)"
```

---

### Task 4: `query_builder.rs`

**Files:**
- Create: `rust/crates/pcbparts-search/src/query_builder.rs`
- Modify: `rust/crates/pcbparts-search/src/lib.rs` (add `pub mod query_builder;`)

**Interfaces:**
- Consumes: `spec_filter::{SpecFilter, SpecOperator, escape_like, generate_value_patterns, get_attribute_names, spec_to_column, SpecParserFn}` (Task 3), `pcbparts_parsers::alternatives::{spec_parsers, SpecParser}` (Phase 2B).
- Produces: `SqlParam` enum (`Text(String)` | `Real(f64)` | `Integer(i64)`), `PostFilterMeta` struct, `build_fts_clause`, `build_subcategory_clause`, `build_library_type_clause`, `build_stock_clause`, `build_package_clause` (+ `expand_package_aliases`), `build_manufacturer_clause`, `build_mounting_type_clause`, `build_spec_filter_clauses` (+ `group_multi_value_filters`), `build_sort_clause`, `needs_numeric_post_filter` — consumed by Task 6's `engine.rs`.

**Deliberate deviation from Python:** SQL parameters are modeled as a concrete `SqlParam` enum (`Text`/`Real`/`Integer`) instead of Python's dynamically-typed list, since Rust needs a concrete type to eventually bind via `rusqlite::ToSql` in Task 6 — same values, same order, just typed.

**Characterization fixtures** (captured by running the live Python `search/query_builder.py`):

```json
{
  "build_fts_clause": {
    "single_term_and": ["\n        AND lcsc IN (\n            SELECT lcsc FROM components_fts\n            WHERE components_fts MATCH ?\n        )\n    ", ["\"resistor\"*"]],
    "multi_term_and": ["<same SQL>", ["\"10k\"* \"resistor\"*"]],
    "multi_term_or": ["<same SQL>", ["\"10k\"* OR \"resistor\"*"]],
    "empty": ["", []],
    "too_long (501 chars)": ["", []],
    "control_char": ["", []]
  },
  "build_subcategory_clause": {
    "by_subcategory_id": ["AND subcategory_id = ?", [1]],
    "by_category_id_with_map": ["AND subcategory_id IN (?,?)", [1, 2]],
    "by_category_id_no_map": ["AND subcategory_id IN (?,?)", [1, 2]],
    "neither": ["", []],
    "category_no_match": ["", []]
  },
  "build_library_type_clause": {
    "basic": "AND library_type = 'b'", "preferred": "AND library_type = 'p'",
    "extended": "AND library_type = 'e'", "no_fee": "AND library_type IN ('b', 'p')",
    "null": "", "bogus": ""
  },
  "build_stock_clause": {"10": ["AND stock >= ?", [10]], "0": ["", []], "-5": ["", []]},
  "build_package_clause": {
    "empty": ["", []],
    "single_sot23": ["AND (package LIKE ? ESCAPE '\\')", ["SOT-23%"]],
    "tqfp44": ["AND (package LIKE ? ESCAPE '\\' OR package LIKE ? ESCAPE '\\' OR package LIKE ? ESCAPE '\\' OR package LIKE ? ESCAPE '\\' OR package LIKE ? ESCAPE '\\')", ["TQFP-44%", "QFP-44%", "LQFP-44%", "PQFP-44%", "HQFP-44%"]]
  },
  "expand_package_aliases": {
    "TQFP-44": ["TQFP-44", "QFP-44", "LQFP-44", "PQFP-44", "HQFP-44"],
    "QFN-56": ["QFN-56", "LQFN-56", "WQFN-56", "VQFN-56", "TQFN-56", "UQFN-56"],
    "UFQFPN-48": ["UFQFPN-48"],
    "SOIC-8": ["SOIC-8", "SOP-8", "SO-8"],
    "SOT-23": ["SOT-23"]
  },
  "build_manufacturer_clause": {"yageo": ["AND LOWER(manufacturer) = LOWER(?)", ["YAGEO"]], "empty": ["", []]},
  "build_mounting_type_clause": {
    "Through Hole": ["AND (description LIKE ? OR description LIKE ?)", ["%Through Hole%", "%Plugin%"]],
    "SMD": ["AND (description LIKE ? OR description LIKE ?)", ["%Surface Mount%", "%SMD%"]],
    "null": ["", []], "bogus": ["", []]
  },
  "group_multi_value_filters_interface": [["Interface", ["I2C", "SPI"]]],
  "build_spec_filter_clauses_resistance_numeric_column": {
    "sql_clauses": ["AND resistance_ohms >= ?"], "params": [10000.0], "post_filter_meta_len": 0
  },
  "build_spec_filter_clauses_interface_grouped": {
    "sql_clauses": ["AND (attributes LIKE ? ESCAPE '\\' OR attributes LIKE ? ESCAPE '\\')"],
    "params": ["%\"Interface\"%I2C%", "%\"Interface\"%SPI%"]
  },
  "build_spec_filter_clauses_string_exact": {
    "sql_clauses": ["AND (attributes LIKE ? ESCAPE '\\')"], "params": ["%\"Type\", \"N-Channel\"%"]
  },
  "build_sort_clause": {
    "price_prefer": "ORDER BY CASE library_type WHEN 'b' THEN 1 WHEN 'p' THEN 2 ELSE 3 END, price ASC NULLS LAST",
    "price_noprefer": "ORDER BY price ASC NULLS LAST",
    "relevance_with_query": "ORDER BY CASE library_type WHEN 'b' THEN 1 WHEN 'p' THEN 2 ELSE 3 END, stock DESC",
    "stock_default": "ORDER BY CASE library_type WHEN 'b' THEN 1 WHEN 'p' THEN 2 ELSE 3 END, stock DESC",
    "stock_noprefer": "ORDER BY stock DESC"
  },
  "needs_numeric_post_filter": {
    "resistance_ge_has_column": false, "type_eq_string_no_parser": false, "vgsth_eq_parser_no_column": true
  }
}
```

- [ ] **Step 1: Write the failing tests**

```rust
// rust/crates/pcbparts-search/src/query_builder.rs — tests module (write this first)
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
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd rust && cargo test -p pcbparts-search query_builder::`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

```rust
// rust/crates/pcbparts-search/src/query_builder.rs — insert above the tests module
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
                            if let Some(SpecParser::Parser(f)) = parsers.get(name.as_str()) {
                                parser = Some(*f);
                                break;
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
                        if let Some(SpecParser::Parser(f)) = parsers.get(name.as_str()) {
                            parser = Some(*f);
                            break;
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
        return attr_names.iter().any(|name| matches!(parsers.get(name.as_str()), Some(SpecParser::Parser(_))));
    }
    false
}
```

Note on `build_subcategory_clause`'s signature: this task models `subcategories` as `&BTreeMap<i64, i64>` (subcategory_id -> category_id) rather than the richer struct `engine.rs` will use internally — Task 6 adapts by building this narrower map from its own `SubcategoryInfo` map before calling this function, or the two can be merged if that proves awkward once `engine.rs` is written (call this out to Task 6's implementer as something to double check, not a hard requirement to keep separate).

- [ ] **Step 4: Run to verify pass**

Run: `cd rust && cargo test -p pcbparts-search query_builder::`
Expected: PASS — 22/22 tests, pristine output.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/pcbparts-search/src/query_builder.rs rust/crates/pcbparts-search/src/lib.rs
git commit -m "rust: port search/query_builder.py (characterization tests, no prior pytest coverage)"
```

---

### Task 5: `result.rs`

**Files:**
- Create: `rust/crates/pcbparts-search/src/result.rs`
- Modify: `rust/crates/pcbparts-search/src/lib.rs` (add `pub mod result;`)

**Interfaces:**
- Consumes: `pcbparts_parsers::mounting::detect_mounting_type` (Phase 2A).
- Produces: `SubcategoryInfo` struct, `row_to_dict(row: &rusqlite::Row, subcategories: &BTreeMap<i64, SubcategoryInfo>) -> serde_json::Value` — consumed by Task 6's `engine.rs`.

**Schema note:** confirmed directly against a real `data/components.db` (`PRAGMA table_info(components)`): columns are `lcsc, mpn, manufacturer, package, stock, library_type, subcategory_id, price, description, attributes, resistance_ohms, ...` (67 columns total; only the first 10 matter for this task, the rest are `SPEC_TO_COLUMN`'s numeric columns Task 4 already handles by name). `attributes` is a JSON array of `[name, value]` string pairs, e.g. `[["Resistance", "10kΩ"], ["Operating Temperature", "-55℃~+155℃"], ...]` — confirmed by querying a real row directly.

**Characterization fixture** (captured from a real row, `lcsc = 'C25804'`, via `row_to_dict` against the real `data/components.db`):

```json
{
  "lcsc": "C25804",
  "model": "0603WAF1002T5E",
  "manufacturer": "UNI-ROYAL(Uniroyal Elec)",
  "package": "0603",
  "stock": 15959990,
  "price": 0.0067,
  "price_10": null,
  "library_type": "basic",
  "preferred": true,
  "category": "Resistors",
  "subcategory": "Chip Resistor - Surface Mount",
  "subcategory_id": 2980,
  "mounting_type": "smd",
  "description": "-55℃~+155℃ 100mW 10kΩ 75V Thick Film Resistor ±1% ±100ppm/℃ 0603 Chip Resistor - Surface Mount ROHS",
  "specs": {
    "Resistance": "10kΩ",
    "Operating Temperature": "-55℃~+155℃",
    "Power(Watts)": "100mW",
    "Type": "Thick Film Resistor",
    "Voltage-Supply(Max)": "75V",
    "Tolerance": "±1%",
    "Temperature Coefficient": "±100ppm/℃"
  }
}
```

Note the `specs` key order above is the exact order the real `attributes` column stores them in — this is precisely why `preserve_order` (Global Constraints, Task 1) is required: without it, `serde_json::Map`'s default `BTreeMap` backing would alphabetize these keys (`Operating Temperature` before `Power(Watts)` before `Resistance`...), which would not match this fixture.

- [ ] **Step 1: Write the failing test**

Since this needs a real `rusqlite::Row` (not trivially constructable outside a live query), the test builds a tiny in-memory SQLite table with the real column shapes and one real row, then runs it through `row_to_dict` and asserts against the fixture above.

```rust
// rust/crates/pcbparts-search/src/result.rs — tests module (write this first)
#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::collections::BTreeMap;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE components (
                lcsc TEXT, mpn TEXT, manufacturer TEXT, package TEXT, stock INTEGER,
                library_type TEXT, subcategory_id INTEGER, price REAL, description TEXT, attributes TEXT
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO components (lcsc, mpn, manufacturer, package, stock, library_type, subcategory_id, price, description, attributes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                "C25804", "0603WAF1002T5E", "UNI-ROYAL(Uniroyal Elec)", "0603", 15959990i64,
                "b", 2980i64, 0.0067, "-55℃~+155℃ 100mW 10kΩ 75V Thick Film Resistor ±1% ±100ppm/℃ 0603 Chip Resistor - Surface Mount ROHS",
                r#"[["Resistance", "10kΩ"], ["Operating Temperature", "-55℃~+155℃"], ["Power(Watts)", "100mW"], ["Type", "Thick Film Resistor"], ["Voltage-Supply(Max)", "75V"], ["Tolerance", "±1%"], ["Temperature Coefficient", "±100ppm/℃"]]"#,
            ],
        )
        .unwrap();
        conn
    }

    fn test_subcategories() -> BTreeMap<i64, SubcategoryInfo> {
        BTreeMap::from([(2980, SubcategoryInfo {
            name: "Chip Resistor - Surface Mount".to_string(),
            category_id: 10,
            category_name: Some("Resistors".to_string()),
        })])
    }

    #[test]
    fn test_row_to_dict_full_shape() {
        let conn = test_conn();
        let subcategories = test_subcategories();
        let mut stmt = conn.prepare("SELECT * FROM components WHERE lcsc = 'C25804'").unwrap();
        let mut rows = stmt.query([]).unwrap();
        let row = rows.next().unwrap().unwrap();
        let dict = row_to_dict(row, &subcategories);

        assert_eq!(dict["lcsc"], "C25804");
        assert_eq!(dict["model"], "0603WAF1002T5E");
        assert_eq!(dict["manufacturer"], "UNI-ROYAL(Uniroyal Elec)");
        assert_eq!(dict["package"], "0603");
        assert_eq!(dict["stock"], 15959990);
        assert_eq!(dict["price"], 0.0067);
        assert_eq!(dict["price_10"], serde_json::Value::Null);
        assert_eq!(dict["library_type"], "basic");
        assert_eq!(dict["preferred"], true);
        assert_eq!(dict["category"], "Resistors");
        assert_eq!(dict["subcategory"], "Chip Resistor - Surface Mount");
        assert_eq!(dict["subcategory_id"], 2980);
        assert_eq!(dict["mounting_type"], "smd");
        assert_eq!(dict["specs"]["Resistance"], "10kΩ");
        assert_eq!(dict["specs"]["Tolerance"], "±1%");

        // Key order must be preserved (JSON array order), not alphabetized —
        // requires the `preserve_order` serde_json feature (Global Constraints).
        let keys: Vec<&String> = dict["specs"].as_object().unwrap().keys().collect();
        assert_eq!(
            keys,
            vec!["Resistance", "Operating Temperature", "Power(Watts)", "Type", "Voltage-Supply(Max)", "Tolerance", "Temperature Coefficient"]
        );
    }

    #[test]
    fn test_row_to_dict_library_type_mapping() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE components (
                lcsc TEXT, mpn TEXT, manufacturer TEXT, package TEXT, stock INTEGER,
                library_type TEXT, subcategory_id INTEGER, price REAL, description TEXT, attributes TEXT
            );
            INSERT INTO components VALUES ('C1', 'M1', 'Mfr', 'SOT-23', 100, 'e', 999, 0.1, 'desc', NULL);",
        )
        .unwrap();
        let subcategories: BTreeMap<i64, SubcategoryInfo> = BTreeMap::new();
        let mut stmt = conn.prepare("SELECT * FROM components").unwrap();
        let mut rows = stmt.query([]).unwrap();
        let row = rows.next().unwrap().unwrap();
        let dict = row_to_dict(row, &subcategories);

        assert_eq!(dict["library_type"], "extended");
        assert_eq!(dict["preferred"], false);
        assert_eq!(dict["category"], serde_json::Value::Null);
        assert_eq!(dict["subcategory"], serde_json::Value::Null);
        assert_eq!(dict["specs"], serde_json::json!({}));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd rust && cargo test -p pcbparts-search result::`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

```rust
// rust/crates/pcbparts-search/src/result.rs — insert above the tests module
use pcbparts_parsers::mounting::detect_mounting_type;
use rusqlite::Row;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub struct SubcategoryInfo {
    pub name: String,
    pub category_id: i64,
    pub category_name: Option<String>,
}

/// Convert a database row to a component dict, matching client.py's `_transform_part()` shape.
pub fn row_to_dict(row: &Row, subcategories: &BTreeMap<i64, SubcategoryInfo>) -> Value {
    let attributes: Option<String> = row.get("attributes").unwrap_or(None);
    let mut specs = serde_json::Map::new();
    if let Some(attrs_json) = attributes.filter(|s| !s.is_empty()) {
        if let Ok(pairs) = serde_json::from_str::<Vec<(String, String)>>(&attrs_json) {
            for (name, value) in pairs {
                specs.insert(name, Value::String(value));
            }
        }
        // Malformed JSON: continue with empty specs, matching Python's
        // `except (json.JSONDecodeError, TypeError)` fallback.
    }

    let library_type_code: String = row.get("library_type").unwrap_or_default();
    let library_type = match library_type_code.as_str() {
        "b" => "basic",
        "p" => "preferred",
        "e" => "extended",
        other => return json!({"_unmapped_library_type": other}), // unreachable with real data; see note below
    };

    let subcategory_id: i64 = row.get("subcategory_id").unwrap_or_default();
    let subcat_info = subcategories.get(&subcategory_id);
    let package: Option<String> = row.get("package").unwrap_or(None);
    let category = subcat_info.and_then(|i| i.category_name.clone());
    let subcategory = subcat_info.map(|i| i.name.clone());

    json!({
        "lcsc": row.get::<_, String>("lcsc").unwrap_or_default(),
        "model": row.get::<_, Option<String>>("mpn").unwrap_or(None),
        "manufacturer": row.get::<_, Option<String>>("manufacturer").unwrap_or(None),
        "package": package,
        "stock": row.get::<_, i64>("stock").unwrap_or_default(),
        "price": row.get::<_, Option<f64>>("price").unwrap_or(None),
        "price_10": Value::Null,
        "library_type": library_type,
        "preferred": library_type_code == "b" || library_type_code == "p",
        "category": category,
        "subcategory": subcategory,
        "subcategory_id": subcategory_id,
        "mounting_type": detect_mounting_type(package.as_deref(), category.as_deref(), subcategory.as_deref()),
        "description": row.get::<_, Option<String>>("description").unwrap_or(None),
        "specs": Value::Object(specs),
    })
}
```

Fix before running tests: the `other => return json!(...)` arm above is wrong — Python's `lib_type_map.get(row["library_type"], row["library_type"])` falls back to the **raw code itself** (not an error marker) for any unrecognized code, and this never actually returns early — it's part of computing a single `library_type: String` value used later in the same dict. Replace that whole `match` with:

```rust
    let library_type = match library_type_code.as_str() {
        "b" => "basic".to_string(),
        "p" => "preferred".to_string(),
        "e" => "extended".to_string(),
        other => other.to_string(),
    };
```

and use `library_type` (owned `String`) in the final `json!` macro's `"library_type": library_type` field. This was a drafting mistake caught during self-review — implement the corrected version, not the one with the early return.

- [ ] **Step 4: Run to verify pass**

Run: `cd rust && cargo test -p pcbparts-search result::`
Expected: PASS — 2/2 tests, pristine output.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/pcbparts-search/src/result.rs rust/crates/pcbparts-search/src/lib.rs
git commit -m "rust: port search/result.py (characterization test against real row shape)"
```

---

### Task 6: `engine.rs`

**Files:**
- Create: `rust/crates/pcbparts-search/src/engine.rs`
- Modify: `rust/crates/pcbparts-search/src/lib.rs` (add `pub mod engine;`)

**Interfaces:**
- Consumes: everything from Tasks 1-5 in this crate, plus `pcbparts_parsers::alternatives::dimension_spec_fields` and `pcbparts_parsers::parsers::parse_dimensions_from_package` (Phase 2A/2B), plus `pcbparts_parsers::subcategory_aliases::{resolve_subcategory_name, find_similar_subcategories, SimilarSubcategory}` (Phase 2A — exact signatures below, verified against the committed source, not guessed).
- Produces: `SearchEngine` struct with `new()`, `resolve_subcategory_name()`, `resolve_category_name()`, `search()` — the crate's primary public surface, consumed by Phase 5's future `ComponentDatabase`.

**Verified Phase 2A signatures this task depends on** (read directly from `rust/crates/pcbparts-parsers/src/subcategory_aliases.rs`, not assumed):
```rust
pub fn resolve_subcategory_name(name: &str, name_to_id: &HashMap<String, i64>, aliases: Option<&HashMap<&str, &str>>) -> Option<i64>;
pub struct SimilarSubcategory { pub id: i64, pub name: String, pub category: String }
pub fn find_similar_subcategories(name: &str, name_to_id: &HashMap<String, i64>, subcategory_info: &HashMap<i64, (String, String)>, limit: usize) -> Vec<SimilarSubcategory>;
```
Note `resolve_subcategory_name` takes `HashMap<String, i64>` (not `BTreeMap`) for `name_to_id` and an `Option<&HashMap<&str, &str>>` for aliases (pass `None` to use the real `subcategory_aliases()` table). `find_similar_subcategories` takes `subcategory_info: &HashMap<i64, (String, String)>` — build this narrower `(name, category_name)` map from `SearchEngine`'s own `subcategories: BTreeMap<i64, SubcategoryInfo>` field when calling it (a small adapter, not a redesign).

**Design decisions locked in by this plan (apply as specified, don't re-derive):**
- `SearchEngine` stores its lookup maps as fields (`subcategories: BTreeMap<i64, SubcategoryInfo>`, `categories: BTreeMap<i64, CategoryInfo>`, `category_to_subcategories: BTreeMap<i64, Vec<i64>>`, `subcategory_name_to_id: HashMap<String, i64>`, `category_name_to_id: HashMap<String, i64>`) but takes `conn: &rusqlite::Connection` as a parameter on `search()` rather than storing it — matching `pcbparts-db`'s existing free-function-over-`&Connection` pattern (`rust/crates/pcbparts-db/src/boards/mod.rs`'s `search::search_boards(&conn, ...)`), and avoiding tying this crate to whatever connection-ownership strategy Phase 5's `ComponentDatabase` ends up using (`Mutex`, pooled, etc.).
- `BTreeMap` (not `HashMap`) for the three maps above: their values get iterated to build ordered SQL param lists (`build_subcategory_clause`'s no-map fallback) or scanned for partial-name matches (`resolve_category_name`) — `BTreeMap`'s deterministic key-order iteration avoids re-introducing the exact non-determinism class Phase 2A found and fixed once already in `resolve_subcategory_name`. `subcategory_name_to_id`/`category_name_to_id` stay `HashMap<String, i64>` to match Phase 2A's `resolve_subcategory_name` signature exactly; `resolve_category_name` (this task's own new function, not delegated to Phase 2A) sorts by `(len, key)` before picking a match, same fix pattern.
- `SearchEngine::search()`'s SQL execution converts `SqlParam` (Task 4) to `rusqlite`-bindable values via `&dyn rusqlite::ToSql`.

**Characterization fixtures** (captured by running `db.search(...)` against the real, live 618,277-part `data/components.db` via the current Python `ComponentDatabase`/`SearchEngine`; each trimmed to metadata + first 1-3 `results` entries):

```json
{
  "plain_fts_query (query='10k 0603 resistor', limit=5, min_stock=10)": {
    "total_ge_1": true,
    "results[0]": {"lcsc": "C25804", "model": "0603WAF1002T5E", "manufacturer": "UNI-ROYAL(Uniroyal Elec)", "package": "0603", "library_type": "basic", "subcategory": "Chip Resistor - Surface Mount", "mounting_type": "smd", "specs.Resistance": "10kΩ"}
  },
  "subcategory_by_name (subcategory_name='MLCC', limit=3, min_stock=10)": {
    "filters_applied.subcategory_resolved": "Multilayer Ceramic Capacitors MLCC - SMD/SMT",
    "results[0].subcategory": "Multilayer Ceramic Capacitors MLCC - SMD/SMT",
    "results[0].subcategory_id": 2929
  },
  "subcategory_by_id (subcategory_id=2929, limit=3, min_stock=10)": {
    "results[0].subcategory_id": 2929
  },
  "category_by_name (category_name='Resistors', limit=3, min_stock=10)": {
    "results[0].category": "Resistors"
  },
  "spec_filter_numeric_column (spec_filters=[SpecFilter('Resistance','=','10k')], limit=3, min_stock=10)": {
    "results[0].specs.Resistance": "10kΩ"
  },
  "spec_filter_ge (spec_filters=[SpecFilter('Voltage Rating','>=','50V')], limit=3, min_stock=10)": {
    "results[0].specs['Voltage Rating']": "50V"
  },
  "spec_filter_parser_no_column (query='mosfet', spec_filters=[SpecFilter('Vgs(th)','<=','3V')], limit=3, min_stock=10)": {
    "results[0].subcategory": "MOSFETs"
  },
  "package_qfn_alias_expansion (package='QFN-32', limit=3, min_stock=10)": {
    "results[0].package": "QFN-32-EP(5x5)"
  },
  "manufacturer_alias (manufacturer='TI', limit=3, min_stock=10)": {
    "results[0].manufacturer": "Texas Instruments"
  },
  "mounting_type_through_hole (query='resistor', mounting_type='Through Hole', limit=3, min_stock=10)": {
    "results[0].mounting_type": "through_hole",
    "results[0].package": "Plugin,D2.7xL6.2mm"
  },
  "prefer_no_fee_false (query='10k resistor', sort_by='price', prefer_no_fee=false, limit=3, min_stock=10)": {
    "results[0].library_type": "extended",
    "results[0].package": "0201"
  },
  "subcategory_not_found (subcategory_name='totally-bogus-subcat-xyz', limit=3)": {
    "error": "Subcategory not found: 'totally-bogus-subcat-xyz'",
    "similar_subcategories": [],
    "total": 0
  },
  "category_not_found (category_name='totally-bogus-category-xyz', limit=3)": {
    "error": "Category not found: 'totally-bogus-category-xyz'",
    "total": 0
  },
  "zero_results (query='zzzznonexistentpartxyz123', limit=3)": {
    "total": 0,
    "results": []
  },
  "mpn_retry_path (query='STM32F103C8T6-TR', limit=3, min_stock=0)": {
    "total": 3,
    "mpn_normalized": {"original_query": "STM32F103C8T6-TR", "matched_query": "STM32F103C8T6", "note": "Original query had no results; found matches using normalized MPN variant"},
    "results[0].model": "STM32F103C8T6"
  }
}
```

The implementer must regenerate the FULL, untrimmed versions of these fixtures before writing tests — this plan shows the shape and key assertions, not the complete JSON (results arrays were trimmed for readability in this document; full field-by-field assertions in the actual Rust tests should come from re-running the equivalent query against a locally rebuilt `data/components.db` and reading the real output, the same way every fixture in this plan was captured). Use this Python one-liner pattern (adjust query/filters per case):

```python
import sys, json
from pathlib import Path
sys.path.insert(0, "src")
from pcbparts_mcp.db import ComponentDatabase
from pcbparts_mcp.search.spec_filter import SpecFilter
db = ComponentDatabase(db_path=Path("data/components.db"), data_dir=Path("data"))
result = db.search(query="10k 0603 resistor", limit=5, min_stock=10)
print(json.dumps(result, indent=2, ensure_ascii=False))
```

- [ ] **Step 1: Write the failing tests**

Because `SearchEngine::search()` needs a real `rusqlite::Connection` against the `components` table's full schema (67 columns) plus an FTS5 virtual table, these tests build a small in-memory fixture database (schema mirrors the real one; a handful of representative rows, not all 618,277) rather than opening `data/components.db` directly — this keeps the test suite fast and hermetic while still exercising real SQL execution (not mocked). Follow `rust/crates/pcbparts-db/src/boards/fixtures.rs`'s pattern for building an in-memory test schema+data if useful as a reference.

```rust
// rust/crates/pcbparts-search/src/engine.rs — tests module (write this first)
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cd rust && cargo test -p pcbparts-search engine::`
Expected: FAIL to compile — `SearchEngine`, `SearchParams`, `SubcategoryInfo` (re-exported from `result.rs`), `CategoryInfo` don't exist yet.

- [ ] **Step 3: Write the implementation**

```rust
// rust/crates/pcbparts-search/src/engine.rs — insert above the tests module
use crate::mpn::{looks_like_mpn, normalize_mpn};
use crate::query_builder::{
    build_fts_clause, build_library_type_clause, build_manufacturer_clause, build_mounting_type_clause,
    build_package_clause, build_sort_clause, build_spec_filter_clauses, build_subcategory_clause,
    needs_numeric_post_filter, SqlParam,
};
use crate::resolvers::{expand_package, resolve_manufacturer};
use crate::result::{row_to_dict, SubcategoryInfo};
use crate::spec_filter::{SpecFilter, SpecParserFn};
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

#[derive(Default)]
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

        let mut query = query.map(|q| crate::resolvers::expand_query_synonyms(&q));

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
```

This is the largest and most intricate task in this plan — it is expected to need at least one review/fix round. Pay particular attention during self-review to: the `SqlParam` → `Box<dyn ToSql>` conversion (params must bind in the exact same order they were pushed), the post-filter loop's early `continue`/`break` semantics matching Python's `for ... else`-free structure, and the `execute_search`'s two-query pattern (main SELECT + a separate library-type-distribution GROUP BY query, both sharing the same WHERE clause minus the `library_type` filter itself).

- [ ] **Step 4: Run to verify pass**

Run: `cd rust && cargo test -p pcbparts-search engine::`
Expected: PASS — 8/8 tests, pristine output.

- [ ] **Step 5: Run the entire `pcbparts-search` crate**

Run: `cd rust && cargo test -p pcbparts-search`
Expected: PASS — 69 tests (15 mpn + 9 resolvers + 13 spec_filter + 22 query_builder + 2 result + 8 engine).

- [ ] **Step 6: Run the entire workspace**

Run: `cd rust && cargo test`
Expected: PASS — 310 tests total (135 Phase 1 `pcbparts-db` + 106 Phase 2A+2B `pcbparts-parsers` + 69 Phase 3 `pcbparts-search`). All previously-shipped phases stay green; this phase adds no regressions.

- [ ] **Step 7: Commit**

```bash
git add rust/crates/pcbparts-search/src/engine.rs rust/crates/pcbparts-search/src/lib.rs
git commit -m "rust: port search/engine.py (SearchEngine, characterization tests against real DB behavior)"
```

## Self-Review Notes

- **Spec coverage:** all 6 Python files (`mpn.py`, `resolvers.py`, `spec_filter.py`, `query_builder.py`, `result.py`, `engine.py`) have a task. The crate's full `search/__init__.py` export list (`SearchEngine`, `SpecFilter`, `SPEC_TO_COLUMN`, `ATTRIBUTE_ALIASES`, `get_attribute_names`, `escape_like`, `expand_query_synonyms`, `expand_package`, `resolve_manufacturer`, `PACKAGE_FAMILIES`, `IMPERIAL_CHIP_SIZES`, `SMD_PACKAGE_FAMILIES`, `normalize_mpn`, `looks_like_mpn`, `row_to_dict`) maps onto: `SearchEngine`→Task 6, `SpecFilter`/`SPEC_TO_COLUMN`/`ATTRIBUTE_ALIASES`/`get_attribute_names`/`escape_like`→Task 3, `expand_query_synonyms`/`expand_package`/`resolve_manufacturer`/`PACKAGE_FAMILIES`/`IMPERIAL_CHIP_SIZES`/`SMD_PACKAGE_FAMILIES`→Task 2, `normalize_mpn`/`looks_like_mpn`→Task 1, `row_to_dict`→Task 5. No `lib.rs` re-export list is specified as a separate step — add `pub use` lines mirroring this list as a final cleanup if the crate needs a flat public surface matching Python's `__init__.py` (not required for this plan's own tests to pass, since intra-crate tests use `crate::module::item` paths directly; flag as a `DONE_WITH_CONCERNS`-worthy note if skipped, not a blocker).
- **Type consistency verified across tasks:** `SpecFilter`/`SpecOperator` (Task 3) are used identically in Tasks 4 and 6. `SqlParam` (Task 4) is produced by `query_builder.rs` and consumed by `engine.rs` (Task 6) with a matching three-variant shape. `SubcategoryInfo` (Task 5) is reused as-is by `engine.rs`'s `SearchEngine` field (Task 6) rather than redefined. `PostFilterMeta` (Task 4) is consumed by `engine.rs`'s post-filter loop (Task 6) using the same field names (`spec_filter`, `attr_names`, `parser`, `target_value`).
- **Placeholder scan:** no TBD/TODO in any task; Task 5 explicitly calls out and corrects one drafting mistake (the `other => return ...` arm) rather than leaving it — port the corrected version, not the first draft shown.
- **No golden Python test exists for `engine.py`'s exact SQL string construction** (only the JSON *output* was characterized, not the intermediate SQL) — Task 6's tests assert on `search()`'s final `Value` output against a small in-memory fixture DB, matching how confidence in this function was actually established (running it and reading its output), not by asserting on private SQL strings.
