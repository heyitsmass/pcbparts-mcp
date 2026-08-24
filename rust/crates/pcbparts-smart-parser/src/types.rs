use pcbparts_parsers::subcategory_aliases::subcategory_aliases;
use regex::Regex;
use std::sync::LazyLock;

// Pre-sorted by length (longest first) for correct matching — a stable sort, matching
// Python's `sorted(SUBCATEGORY_ALIASES.keys(), key=len, reverse=True)`.
static SUBCATEGORY_KEYWORDS_BY_LENGTH: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut keys: Vec<&'static str> = subcategory_aliases().into_keys().collect();
    // Sort by length (descending), then alphabetically (ascending) as tiebreak.
    // HashMap iteration order is not stable across process runs, so a deterministic
    // secondary sort key (lexicographic string comparison) is required for reproducible behavior.
    keys.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
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
}
