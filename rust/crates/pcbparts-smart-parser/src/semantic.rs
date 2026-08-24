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
