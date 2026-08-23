use pcbparts_parsers::alternatives::spec_parsers;
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
    if spec_parsers().contains_key(name) {
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
