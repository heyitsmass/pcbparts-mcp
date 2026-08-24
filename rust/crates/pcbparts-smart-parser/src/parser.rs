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
