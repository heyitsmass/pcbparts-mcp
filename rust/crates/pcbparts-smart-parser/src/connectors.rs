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
