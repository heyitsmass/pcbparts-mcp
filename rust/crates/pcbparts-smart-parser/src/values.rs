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
        let pins: i64 = caps[1].parse().unwrap_or(i64::MAX);
        extractions.push((m.start(), m.end(), ExtractedValue::new(m.as_str(), pins as f64, "pin_count", format!("{pins}P"))));
    }

    // Position count (for connectors: 2-pos, 2 position, 2-way, 2P)
    for caps in POSITION.captures_iter(query) {
        let m = caps.get(0).unwrap();
        if overlaps(&extractions, m.start()) {
            continue;
        }
        let positions: i64 = caps[1].parse().unwrap_or(i64::MAX);
        extractions.push((m.start(), m.end(), ExtractedValue::new(m.as_str(), positions as f64, "position_count", format!("{positions}P"))));
    }

    // Pin structure for headers (1x7, 2x20, etc.) — maps to "Pin Structure", not
    // "Number of Pins"
    for caps in PIN_STRUCTURE.captures_iter(query) {
        let m = caps.get(0).unwrap();
        if overlaps(&extractions, m.start()) {
            continue;
        }
        let rows: i64 = caps[1].parse().unwrap_or(i64::MAX);
        let pins_per_row: i64 = caps[2].parse().unwrap_or(i64::MAX);
        let total = rows.saturating_mul(pins_per_row);
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

#[cfg(test)]
mod tests {
    use super::extract_values;

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

    #[test]
    fn pin_count_pathologically_long_digits() {
        // 25-digit number exceeds i64::MAX (which is 9223372036854775807, 19 digits)
        // Parser should saturate to i64::MAX instead of panicking
        let (values, _remaining) = extract_values("12345678901234567890123456 pin header");
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].unit_type, "pin_count");
        // The saturated value i64::MAX as f64
        approx(values[0].value, i64::MAX as f64);
        assert_eq!(values[0].normalized, "9223372036854775807P");
    }

    #[test]
    fn position_count_pathologically_long_digits() {
        // 25-digit number exceeds i64::MAX
        // Parser should saturate to i64::MAX instead of panicking
        let (values, _remaining) = extract_values("12345678901234567890123456 position connector");
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].unit_type, "position_count");
        // The saturated value i64::MAX as f64
        approx(values[0].value, i64::MAX as f64);
    }

    #[test]
    fn pin_structure_pathologically_long_digits() {
        // PIN_STRUCTURE pattern matches ([12])\s*[xX]\s*(\d+)
        // First group is constrained to [12], but second group can be pathologically long
        // Parser should saturate to i64::MAX instead of panicking
        let (values, _remaining) = extract_values("2x12345678901234567890123456 pin header");
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].unit_type, "pin_structure");
        // rows=2, pins_per_row saturates to i64::MAX, total = 2 * i64::MAX (wraps, but doesn't panic)
        // The saturated multiplication doesn't panic because we're casting i64 to f64
        assert!(values[0].value.is_finite());
    }
}
