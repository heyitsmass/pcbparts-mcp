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
