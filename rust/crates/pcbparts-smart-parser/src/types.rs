use pcbparts_parsers::subcategory_aliases::subcategory_aliases;
use regex::Regex;
use std::sync::LazyLock;

// Precompiled once (keyword regex, keyword text, mapped subcategory), pre-sorted by
// length (longest first) for correct matching — a stable sort, matching Python's
// `sorted(SUBCATEGORY_ALIASES.keys(), key=len, reverse=True)`. Same pattern as
// `semantic.rs`'s `SORTED_DESCRIPTORS`: compiling every keyword's regex and rebuilding
// the alias HashMap on every call was a 45x perf regression vs Python (see
// final-review-report.md finding #1) — both are now built once at first use.
static SUBCATEGORY_KEYWORD_PATTERNS: LazyLock<Vec<(Regex, &'static str, &'static str)>> = LazyLock::new(|| {
    let aliases = subcategory_aliases();
    let mut keys: Vec<&'static str> = aliases.keys().copied().collect();
    // Sort by length (descending), then alphabetically (ascending) as tiebreak.
    // HashMap iteration order is not stable across process runs, so a deterministic
    // secondary sort key (lexicographic string comparison) is required for reproducible
    // behavior.
    //
    // NOTE: like `pcbparts-parsers/src/subcategory_aliases.rs:456-458`'s
    // `find_subcategory_id` tiebreak, this alphabetical tiebreak intentionally diverges
    // from Python's insertion-order tiebreak for equal-length keyword ties (e.g. "diode
    // boost": Python picks "diode" -> "switching diodes" by insertion order, this picks
    // "boost" -> "dc-dc converters" alphabetically). See final-review-report.md finding
    // #5 (PARKED) — a cross-crate ordered-accessor fix to reproduce Python's tiebreak
    // exactly was ruled out of scope for this fix round.
    keys.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    keys.into_iter()
        .map(|keyword| {
            // Word boundaries avoid "sram" matching inside "PSRAM".
            let pattern = Regex::new(&format!(r"(?i)\b{}\b", regex::escape(keyword))).unwrap();
            (pattern, keyword, aliases[keyword])
        })
        .collect()
});

static WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// Extract component type from `query`. Returns `(subcategory_name, remaining_query,
/// matched_keyword)`.
pub fn extract_component_type(query: &str) -> (Option<String>, String, Option<String>) {
    let query_lower = query.to_lowercase();

    for (pattern, keyword, subcategory) in SUBCATEGORY_KEYWORD_PATTERNS.iter() {
        if pattern.is_match(&query_lower) {
            let remaining = pattern.replace_all(query, "").trim().to_string();
            let remaining = WHITESPACE_RE.replace_all(&remaining, " ").to_string();
            return (Some(subcategory.to_string()), remaining, Some(keyword.to_string()));
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

    #[test]
    fn same_length_keyword_tiebreak_determinism() {
        // Regression test for non-deterministic tiebreak in keyword sort.
        // "cap" and "pot" are both length 3 and will collide on length comparison.
        // With deterministic tiebreak (alphabetical), "cap" always wins.
        // Verify that calling extract_component_type twice with a query containing
        // both keywords returns the same result both times (deterministic behavior).
        let query = "cap and pot test";
        let result1 = extract_component_type(query);
        let result2 = extract_component_type(query);
        assert_eq!(result1, result2, "extract_component_type must be deterministic across calls");
        // Verify it matched "cap" (comes before "pot" alphabetically).
        assert_eq!(
            result1.2,
            Some("cap".to_string()),
            "deterministic tiebreak should match 'cap' before 'pot' alphabetically"
        );
    }

    #[test]
    fn tiebreak_diverges_from_python_insertion_order_by_design() {
        // "diode" and "boost" are both length-5 keywords that collide in the
        // length-bucket sort. Python's insertion-order tiebreak picks "diode"
        // (declared earlier in subcategory_aliases.rs) -> "switching diodes".
        // Verified against the live Python `parse_smart_query("diode boost")`.
        // This crate's deterministic alphabetical tiebreak instead picks "boost"
        // (alphabetically first) -> "dc-dc converters". This is an intentional,
        // documented divergence from Python (final-review-report.md finding #5,
        // PARKED) — pinning the *chosen* alphabetical behavior here so a future
        // change can't silently flip it back without review.
        let (subcat, _remaining, kw) = extract_component_type("diode boost");
        assert_eq!(kw, Some("boost".to_string()));
        assert_eq!(subcat, Some("dc-dc converters".to_string()));
    }

    #[test]
    fn extract_component_type_is_fast_when_precompiled() {
        // Regression guard for finding #1 (measured 45x perf regression in release mode
        // from rebuilding the ~370-entry keyword regex list + alias HashMap on every
        // call — 16.1ms/call vs 0.365ms/call). `cargo test` runs unoptimized by default,
        // so this bound is calibrated generously against debug-mode reality (measured
        // ~150-300µs/call here) rather than release-mode cache-hit cost: a
        // reintroduction of the per-call-rebuild bug would cost many *seconds* for 500
        // calls even in an optimized build, so 2s total gives large margin on both sides
        // without being flaky under CI/debug-build slowness.
        let start = std::time::Instant::now();
        for _ in 0..500 {
            let _ = extract_component_type("10k resistor 0603 1% low power");
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 2000,
            "500 calls to extract_component_type took {elapsed:?}, expected well under 2s \
             (regex/HashMap rebuild-per-call regression?)"
        );
    }
}
