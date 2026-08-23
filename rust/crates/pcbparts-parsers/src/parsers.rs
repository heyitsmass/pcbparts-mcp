use regex::Regex;
use std::sync::LazyLock;

static VOLTAGE_KV_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*kV").unwrap());
static VOLTAGE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*V").unwrap());
static TOLERANCE_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([\d.]+)\s*%").unwrap());
static PPM_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[±]?([\d.]+)\s*ppm").unwrap());
static VF_AT_IF_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*mV\s*@").unwrap());
static POWER_FRACTION_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(\d+)/(\d+)\s*W").unwrap());
static POWER_MW_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*mW").unwrap());
static POWER_W_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*W").unwrap());
static CURRENT_UA_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*[uµ]A").unwrap());
static CURRENT_MA_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*mA").unwrap());
static CURRENT_A_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*A").unwrap());
static RESISTANCE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([\d.]+)\s*([kKmM])?").unwrap());
static RESISTANCE_EURO_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(\d+)([kKrR])(\d+)|(\d+)(M)(\d+)").unwrap());
static CAPACITANCE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*([pnuµm])?").unwrap());
static INDUCTANCE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*([nuµm])?").unwrap());
static FREQUENCY_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([\d.]+)\s*([kKmMgG])?").unwrap());
static IMPEDANCE_AT_FREQ_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)([\d.]+)\s*([kKmM])?Ohm\s*@\s*([\d.]+)\s*([kKmMgG])?Hz").unwrap()
});
static DECIBEL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*dB").unwrap());
static TEMPERATURE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([+-]?[\d.]+)\s*[°℃]?C?").unwrap());
static TEMP_RANGE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)([+-]?\d+)\s*[°℃]?C?\s*~\s*[+]?([+-]?\d+)\s*[°℃]?C?").unwrap()
});
static MEMORY_BIT_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*([KMG])?BIT").unwrap());
static MEMORY_BYTE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*([KMG])?B").unwrap());
static WAVELENGTH_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*nm").unwrap());
static LUMINOSITY_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*mcd").unwrap());
static CAPACITANCE_PF_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*([pn])?").unwrap());
static LENGTH_MM_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*mm").unwrap());
static INTEGER_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+)").unwrap());
static VGS_RANGE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([\d.]+)\s*V?\s*~\s*([\d.]+)\s*V?").unwrap());
static FREQ_RANGE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)([\d.]+)\s*([kKmMgG])?Hz?\s*~\s*([\d.]+)\s*([kKmMgG])?Hz?").unwrap()
});
static PACKAGE_DIMENSIONS_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)D\s*(\d+(?:\.\d+)?)\s*[x×X]\s*L?\s*(\d+(?:\.\d+)?)\s*mm").unwrap()
});

pub fn parse_voltage(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    if let Some(c) = VOLTAGE_KV_PATTERN.captures(s) {
        return Some(c[1].parse::<f64>().unwrap() * 1000.0);
    }
    VOLTAGE_PATTERN.captures(s).map(|c| c[1].parse().unwrap())
}

pub fn parse_tolerance(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    TOLERANCE_PATTERN.captures(s).map(|c| c[1].parse().unwrap())
}

pub fn parse_ppm(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    PPM_PATTERN.captures(s).map(|c| c[1].parse().unwrap())
}

pub fn parse_forward_voltage(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    if let Some(c) = VF_AT_IF_PATTERN.captures(s) {
        return Some(c[1].parse::<f64>().unwrap() / 1000.0);
    }
    VOLTAGE_PATTERN.captures(s).map(|c| c[1].parse().unwrap())
}

pub fn parse_power(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    if let Some(c) = POWER_FRACTION_PATTERN.captures(s) {
        let num: f64 = c[1].parse().unwrap();
        let den: f64 = c[2].parse().unwrap();
        return Some(num / den);
    }
    if let Some(c) = POWER_MW_PATTERN.captures(s) {
        return Some(c[1].parse::<f64>().unwrap() / 1000.0);
    }
    if let Some(c) = POWER_W_PATTERN.captures(s) {
        return Some(c[1].parse().unwrap());
    }
    None
}

pub fn parse_current(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    if let Some(c) = CURRENT_UA_PATTERN.captures(s) {
        return Some(c[1].parse::<f64>().unwrap() / 1_000_000.0);
    }
    if let Some(c) = CURRENT_MA_PATTERN.captures(s) {
        return Some(c[1].parse::<f64>().unwrap() / 1000.0);
    }
    if let Some(c) = CURRENT_A_PATTERN.captures(s) {
        return Some(c[1].parse().unwrap());
    }
    None
}

pub fn parse_resistance(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }

    // "mΩ" is case-sensitive in Python (milli-ohm symbol); "mohm" is checked
    // case-insensitively via s.lower(), matching that split exactly.
    let is_milliohm = s.contains("mΩ") || s.to_lowercase().contains("mohm");

    // Python's `.replace("ohm", "")` is case-sensitive (lowercase "ohm" only) —
    // mixed-case "Ohm"/"OHM" survive this strip, same as the source.
    let s_clean = s
        .replace('Ω', "")
        .replace("\\u03a9", "")
        .replace("ohm", "")
        .trim()
        .to_string();

    if !is_milliohm {
        if let Some(c) = RESISTANCE_EURO_PATTERN.captures(&s_clean) {
            let (int_part, suffix, frac_part) = if c.get(1).is_some() {
                (c[1].to_string(), c[2].to_uppercase(), c[3].to_string())
            } else {
                (c[4].to_string(), c[5].to_uppercase(), c[6].to_string())
            };
            let value: f64 = format!("{int_part}.{frac_part}").parse().unwrap();
            return Some(match suffix.as_str() {
                "R" => value,
                "K" => value * 1000.0,
                "M" => value * 1_000_000.0,
                _ => value,
            });
        }
    }

    if s_clean.to_uppercase() == "0R" || s_clean == "0" {
        return Some(0.0);
    }

    let m = RESISTANCE_PATTERN.captures(&s_clean)?;
    let value: f64 = m[1].parse().unwrap();

    if is_milliohm {
        return Some(value / 1000.0);
    }

    let suffix = m.get(2).map(|m| m.as_str().to_uppercase()).unwrap_or_default();
    Some(match suffix.as_str() {
        "K" => value * 1000.0,
        "M" => value * 1_000_000.0,
        _ => value,
    })
}

pub fn parse_capacitance(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    let s = s.replace('F', "");
    let s = s.trim();
    let m = CAPACITANCE_PATTERN.captures(s)?;
    let value: f64 = m[1].parse().unwrap();
    let suffix = m.get(2).map(|m| m.as_str().to_lowercase()).unwrap_or_default();
    Some(match suffix.as_str() {
        "p" => value * 1e-12,
        "n" => value * 1e-9,
        "u" | "µ" => value * 1e-6,
        "m" => value * 1e-3,
        _ => value,
    })
}

pub fn parse_inductance(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    let s = s.replace('H', "");
    let s = s.trim();
    let m = INDUCTANCE_PATTERN.captures(s)?;
    let value: f64 = m[1].parse().unwrap();
    let suffix = m.get(2).map(|m| m.as_str().to_lowercase()).unwrap_or_default();
    Some(match suffix.as_str() {
        "n" => value * 1e-9,
        "u" | "µ" => value * 1e-6,
        "m" => value * 1e-3,
        _ => value,
    })
}

pub fn parse_frequency(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    let s = s.replace("Hz", "");
    let s = s.trim();
    let m = FREQUENCY_PATTERN.captures(s)?;
    let value: f64 = m[1].parse().unwrap();
    let suffix = m.get(2).map(|m| m.as_str().to_uppercase()).unwrap_or_default();
    Some(match suffix.as_str() {
        "K" => value * 1e3,
        "M" => value * 1e6,
        "G" => value * 1e9,
        _ => value,
    })
}

pub fn parse_decibels(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    DECIBEL_PATTERN.captures(s).map(|c| c[1].parse().unwrap())
}

pub fn parse_temperature(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    TEMPERATURE_PATTERN.captures(s).map(|c| c[1].parse().unwrap())
}

pub fn parse_temp_range(s: &str) -> (Option<f64>, Option<f64>) {
    if s.is_empty() {
        return (None, None);
    }
    match TEMP_RANGE_PATTERN.captures(s) {
        Some(c) => (Some(c[1].parse().unwrap()), Some(c[2].parse().unwrap())),
        None => (None, None),
    }
}

pub fn parse_memory_size(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    let s_upper = s.to_uppercase();

    if let Some(c) = MEMORY_BIT_PATTERN.captures(&s_upper) {
        let mut value: f64 = c[1].parse().unwrap();
        let suffix = c.get(2).map(|m| m.as_str()).unwrap_or("");
        value *= match suffix {
            "K" => 1024.0,
            "M" => 1024.0 * 1024.0,
            "G" => 1024.0 * 1024.0 * 1024.0,
            _ => 1.0,
        };
        return Some(value / 8.0);
    }

    if let Some(c) = MEMORY_BYTE_PATTERN.captures(&s_upper) {
        let mut value: f64 = c[1].parse().unwrap();
        let suffix = c.get(2).map(|m| m.as_str()).unwrap_or("");
        value *= match suffix {
            "K" => 1024.0,
            "M" => 1024.0 * 1024.0,
            "G" => 1024.0 * 1024.0 * 1024.0,
            _ => 1.0,
        };
        return Some(value);
    }

    None
}

pub fn parse_percentage(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    TOLERANCE_PATTERN.captures(s).map(|c| c[1].parse().unwrap())
}

pub fn parse_wavelength(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    WAVELENGTH_PATTERN.captures(s).map(|c| c[1].parse().unwrap())
}

pub fn parse_luminosity(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    LUMINOSITY_PATTERN.captures(s).map(|c| c[1].parse().unwrap())
}

pub fn parse_capacitance_pf(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    let s = s.replace('F', "");
    let s = s.trim();
    let m = CAPACITANCE_PF_PATTERN.captures(s)?;
    let value: f64 = m[1].parse().unwrap();
    let suffix = m.get(2).map(|m| m.as_str().to_lowercase()).unwrap_or_default();
    Some(if suffix == "n" { value * 1000.0 } else { value })
}

pub fn parse_length_mm(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    LENGTH_MM_PATTERN.captures(s).map(|c| c[1].parse().unwrap())
}

pub fn parse_dimensions_from_package(pkg: &str) -> (Option<f64>, Option<f64>) {
    if pkg.is_empty() {
        return (None, None);
    }
    match PACKAGE_DIMENSIONS_PATTERN.captures(pkg) {
        Some(c) => (Some(c[1].parse().unwrap()), Some(c[2].parse().unwrap())),
        None => (None, None),
    }
}

pub fn parse_integer(s: &str) -> Option<i64> {
    if s.is_empty() {
        return None;
    }
    INTEGER_PATTERN.captures(s).and_then(|c| c[1].parse::<i128>().ok().and_then(|v| i64::try_from(v).ok()))
}

pub fn parse_vgs_range(s: &str) -> (Option<f64>, Option<f64>) {
    if s.is_empty() {
        return (None, None);
    }
    if let Some(c) = VGS_RANGE_PATTERN.captures(s) {
        return (Some(c[1].parse().unwrap()), Some(c[2].parse().unwrap()));
    }
    if let Some(c) = VOLTAGE_PATTERN.captures(s) {
        let val: f64 = c[1].parse().unwrap();
        return (Some(val), Some(val));
    }
    (None, None)
}

pub fn parse_freq_range(s: &str) -> (Option<f64>, Option<f64>) {
    if s.is_empty() {
        return (None, None);
    }
    if let Some(c) = FREQ_RANGE_PATTERN.captures(s) {
        let val1: f64 = c[1].parse().unwrap();
        let suffix1 = c.get(2).map(|m| m.as_str().to_uppercase()).unwrap_or_default();
        let val2: f64 = c[3].parse().unwrap();
        let suffix2 = c.get(4).map(|m| m.as_str().to_uppercase()).unwrap_or_default();
        let mult = |s: &str| match s {
            "K" => 1e3,
            "M" => 1e6,
            "G" => 1e9,
            _ => 1.0,
        };
        return (Some(val1 * mult(&suffix1)), Some(val2 * mult(&suffix2)));
    }
    let single = parse_frequency(s);
    (single, single)
}

pub fn parse_vin_range(s: &str) -> (Option<f64>, Option<f64>) {
    if s.is_empty() {
        return (None, None);
    }
    if let Some(c) = VGS_RANGE_PATTERN.captures(s) {
        return (Some(c[1].parse().unwrap()), Some(c[2].parse().unwrap()));
    }
    let single = parse_voltage(s);
    (single, single)
}

pub fn parse_impedance_at_freq(s: &str) -> Option<(f64, f64)> {
    if s.is_empty() {
        return None;
    }
    let s = s.replace('Ω', "Ohm").replace("ohm", "Ohm");
    let m = IMPEDANCE_AT_FREQ_PATTERN.captures(&s)?;

    let mut imp_value: f64 = m[1].parse().unwrap();
    let imp_suffix = m.get(2).map(|m| m.as_str().to_uppercase()).unwrap_or_default();
    if imp_suffix == "K" {
        imp_value *= 1000.0;
    } else if imp_suffix == "M" {
        imp_value *= 1_000_000.0;
    }

    let mut freq_value: f64 = m[3].parse().unwrap();
    let freq_suffix = m.get(4).map(|m| m.as_str().to_uppercase()).unwrap_or_default();
    freq_value *= match freq_suffix.as_str() {
        "K" => 1e3,
        "M" => 1e6,
        "G" => 1e9,
        _ => 1.0,
    };

    Some((imp_value, freq_value))
}

pub fn impedance_at_freq_match(orig: &str, cand: &str) -> bool {
    let orig_parsed = parse_impedance_at_freq(orig);
    let cand_parsed = parse_impedance_at_freq(cand);

    let (orig_imp, orig_freq) = match orig_parsed {
        Some(v) => v,
        None => {
            return orig.replace('Ω', "Ohm").to_lowercase() == cand.replace('Ω', "Ohm").to_lowercase();
        }
    };
    let (cand_imp, cand_freq) = match cand_parsed {
        Some(v) => v,
        None => {
            return orig.replace('Ω', "Ohm").to_lowercase() == cand.replace('Ω', "Ohm").to_lowercase();
        }
    };

    if orig_imp == 0.0 || orig_freq == 0.0 {
        return cand_imp == orig_imp && cand_freq == orig_freq;
    }

    let imp_ok = (orig_imp - cand_imp).abs() / orig_imp < 0.02;
    let freq_ok = (orig_freq - cand_freq).abs() / orig_freq < 0.02;
    imp_ok && freq_ok
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} != {b}");
    }

    // === test_parsers.py (parsers.py-only portion) ===

    #[test]
    fn european_notation() {
        for (input, expected) in [
            ("4k7", 4700.0), ("4K7", 4700.0), ("10k0", 10000.0), ("1k0", 1000.0),
            ("4R7", 4.7), ("4r7", 4.7), ("470R", 470.0), ("470r", 470.0),
            ("0R", 0.0), ("0r", 0.0), ("1M5", 1500000.0), ("1m5", 1500000.0), ("2M2", 2200000.0),
        ] {
            approx(parse_resistance(input).unwrap(), expected);
        }
    }

    #[test]
    fn milliohm() {
        for (input, expected) in [
            ("17mΩ", 0.017), ("17mohm", 0.017), ("100mΩ", 0.1), ("100mohm", 0.1),
            ("1mΩ", 0.001), ("50mOhm", 0.05),
        ] {
            approx(parse_resistance(input).unwrap(), expected);
        }
    }

    #[test]
    fn standard_notation() {
        for (input, expected) in [
            ("10k", 10000.0), ("10K", 10000.0), ("4.7k", 4700.0), ("4.7K", 4700.0),
            ("100", 100.0), ("470", 470.0), ("1M", 1000000.0), ("2.2M", 2200000.0),
            ("10kΩ", 10000.0), ("100Ω", 100.0), ("1MΩ", 1000000.0),
            ("10kohm", 10000.0), ("100ohm", 100.0),
        ] {
            approx(parse_resistance(input).unwrap(), expected);
        }
    }

    #[test]
    fn jumper_zero_ohm() {
        assert_eq!(parse_resistance("0R"), Some(0.0));
        assert_eq!(parse_resistance("0r"), Some(0.0));
        assert_eq!(parse_resistance("0"), Some(0.0));
        assert_eq!(parse_resistance("0Ω"), Some(0.0));
        assert_eq!(parse_resistance("0 ohm"), Some(0.0));
    }

    #[test]
    fn resistance_empty_returns_none() {
        assert_eq!(parse_resistance(""), None);
    }

    #[test]
    fn capacitance_parsing() {
        for (input, expected) in [
            ("100uF", 100e-6), ("100µF", 100e-6), ("10uF", 10e-6), ("4.7uF", 4.7e-6),
            ("100nF", 100e-9), ("10nF", 10e-9), ("100pF", 100e-12), ("10pF", 10e-12), ("1mF", 1e-3),
        ] {
            approx(parse_capacitance(input).unwrap(), expected);
        }
    }

    #[test]
    fn voltage_parsing() {
        for (input, expected) in [
            ("5V", 5.0), ("3.3V", 3.3), ("12V", 12.0), ("50V", 50.0),
            ("1kV", 1000.0), ("2.5kV", 2500.0), ("6.3v", 6.3),
        ] {
            approx(parse_voltage(input).unwrap(), expected);
        }
    }

    #[test]
    fn current_parsing() {
        for (input, expected) in [
            ("2A", 2.0), ("5A", 5.0), ("500mA", 0.5), ("100mA", 0.1), ("100uA", 0.0001), ("50µA", 0.00005),
        ] {
            approx(parse_current(input).unwrap(), expected);
        }
    }

    #[test]
    fn tolerance_parsing() {
        for (input, expected) in [
            ("1%", 1.0), ("5%", 5.0), ("10%", 10.0), ("0.1%", 0.1), ("±1%", 1.0), ("±5%", 5.0),
        ] {
            approx(parse_tolerance(input).unwrap(), expected);
        }
    }

    #[test]
    fn power_parsing() {
        for (input, expected) in [
            ("1W", 1.0), ("2W", 2.0), ("100mW", 0.1), ("250mW", 0.25),
            ("1/4W", 0.25), ("1/8W", 0.125), ("1/10W", 0.1),
        ] {
            approx(parse_power(input).unwrap(), expected);
        }
    }

    #[test]
    fn length_parsing() {
        for (input, expected) in [
            ("5.4mm", 5.4), ("4mm", 4.0), ("10.2mm", 10.2), ("21.5mm", 21.5), ("6.3 mm", 6.3), ("5.4MM", 5.4),
        ] {
            approx(parse_length_mm(input).unwrap(), expected);
        }
    }

    #[test]
    fn null_sentinels_return_none() {
        for input in ["-", "", "SMD", "N/A", "unknown"] {
            assert_eq!(parse_length_mm(input), None);
        }
    }

    #[test]
    fn dim_extraction() {
        for (pkg, expected) in [
            ("SMD,D6.3xL5.7mm", (6.3, 5.7)),
            ("SMD,D10xL10.2mm", (10.0, 10.2)),
            ("D8xL12.5mm", (8.0, 12.5)),
            ("SMD, D5 x L5.4 mm", (5.0, 5.4)),
            ("D6.3×L7.7mm", (6.3, 7.7)),
            ("SMD,d6.3xl5.8MM", (6.3, 5.8)),
        ] {
            let got = parse_dimensions_from_package(pkg);
            approx(got.0.unwrap(), expected.0);
            approx(got.1.unwrap(), expected.1);
        }
    }

    #[test]
    fn no_dims_returns_none_pair() {
        for pkg in ["SMD", "", "TO-220", "0805", "SOT-23-3", "radial"] {
            assert_eq!(parse_dimensions_from_package(pkg), (None, None));
        }
    }

    #[test]
    fn memory_size_parsing() {
        for (input, expected) in [
            ("128KB", 131072.0), ("256KB", 262144.0), ("1MB", 1048576.0), ("2MB", 2097152.0),
            ("128Mbit", 16777216.0), ("64Kbit", 8192.0),
        ] {
            approx(parse_memory_size(input).unwrap(), expected);
        }
    }

    // === test_alternatives.py (re-exported parser tests) ===

    #[test]
    fn alt_voltage_simple() {
        assert_eq!(parse_voltage("25V"), Some(25.0));
        assert_eq!(parse_voltage("6.3V"), Some(6.3));
        assert_eq!(parse_voltage("3.3V"), Some(3.3));
        assert_eq!(parse_voltage("50V"), Some(50.0));
    }

    #[test]
    fn alt_voltage_with_spaces() {
        assert_eq!(parse_voltage("25 V"), Some(25.0));
        assert_eq!(parse_voltage("6.3 V"), Some(6.3));
    }

    #[test]
    fn alt_voltage_case_insensitive() {
        assert_eq!(parse_voltage("25v"), Some(25.0));
        assert_eq!(parse_voltage("25V"), Some(25.0));
    }

    #[test]
    fn alt_voltage_kilovolts() {
        assert_eq!(parse_voltage("5kV"), Some(5000.0));
        approx(parse_voltage("3.75kV").unwrap(), 3750.0);
        approx(parse_voltage("1.5 kV").unwrap(), 1500.0);
    }

    #[test]
    fn alt_voltage_invalid() {
        assert_eq!(parse_voltage(""), None);
        assert_eq!(parse_voltage("abc"), None);
    }

    #[test]
    fn alt_tolerance() {
        assert_eq!(parse_tolerance("±1%"), Some(1.0));
        assert_eq!(parse_tolerance("±10%"), Some(10.0));
        assert_eq!(parse_tolerance("±0.5%"), Some(0.5));
        assert_eq!(parse_tolerance("1%"), Some(1.0));
        assert_eq!(parse_tolerance("5%"), Some(5.0));
        assert_eq!(parse_tolerance(""), None);
        assert_eq!(parse_tolerance("abc"), None);
    }

    #[test]
    fn alt_ppm() {
        assert_eq!(parse_ppm("±20ppm"), Some(20.0));
        assert_eq!(parse_ppm("±10ppm"), Some(10.0));
        assert_eq!(parse_ppm("±50ppm"), Some(50.0));
        assert_eq!(parse_ppm("20ppm"), Some(20.0));
        assert_eq!(parse_ppm("30ppm"), Some(30.0));
        assert_eq!(parse_ppm("20 ppm"), Some(20.0));
        assert_eq!(parse_ppm("±10 ppm"), Some(10.0));
        assert_eq!(parse_ppm("20PPM"), Some(20.0));
        assert_eq!(parse_ppm("20Ppm"), Some(20.0));
        assert_eq!(parse_ppm(""), None);
        assert_eq!(parse_ppm("abc"), None);
        assert_eq!(parse_ppm("20%"), None);
    }

    #[test]
    fn alt_forward_voltage() {
        approx(parse_forward_voltage("550mV@3A").unwrap(), 0.55);
        approx(parse_forward_voltage("350mV@1A").unwrap(), 0.35);
        approx(parse_forward_voltage("600 mV @ 2A").unwrap(), 0.6);
        assert_eq!(parse_forward_voltage("1V@100mA"), Some(1.0));
        assert_eq!(parse_forward_voltage("1.2V@20mA"), Some(1.2));
        assert_eq!(parse_forward_voltage("3.3V@10mA"), Some(3.3));
        assert_eq!(parse_forward_voltage(""), None);
        assert_eq!(parse_forward_voltage("abc"), None);
    }

    #[test]
    fn alt_power() {
        assert_eq!(parse_power("1W"), Some(1.0));
        assert_eq!(parse_power("0.25W"), Some(0.25));
        assert_eq!(parse_power("2.5W"), Some(2.5));
        approx(parse_power("100mW").unwrap(), 0.1);
        approx(parse_power("250mW").unwrap(), 0.25);
        assert_eq!(parse_power("1/4W"), Some(0.25));
        assert_eq!(parse_power("1/10W"), Some(0.1));
        assert_eq!(parse_power("1/2W"), Some(0.5));
        assert_eq!(parse_power(""), None);
        assert_eq!(parse_power("abc"), None);
    }

    #[test]
    fn alt_current() {
        assert_eq!(parse_current("2A"), Some(2.0));
        assert_eq!(parse_current("0.5A"), Some(0.5));
        assert_eq!(parse_current("500mA"), Some(0.5));
        approx(parse_current("100mA").unwrap(), 0.1);
        approx(parse_current("100uA").unwrap(), 0.0001);
        approx(parse_current("100µA").unwrap(), 0.0001);
        assert_eq!(parse_current(""), None);
        assert_eq!(parse_current("abc"), None);
    }

    #[test]
    fn alt_resistance() {
        assert_eq!(parse_resistance("10Ω"), Some(10.0));
        assert_eq!(parse_resistance("100"), Some(100.0));
        assert_eq!(parse_resistance("4.7Ω"), Some(4.7));
        assert_eq!(parse_resistance("10kΩ"), Some(10000.0));
        assert_eq!(parse_resistance("10K"), Some(10000.0));
        assert_eq!(parse_resistance("4.7k"), Some(4700.0));
        assert_eq!(parse_resistance("1MΩ"), Some(1_000_000.0));
        assert_eq!(parse_resistance("4.7M"), Some(4_700_000.0));
        assert_eq!(parse_resistance(""), None);
        assert_eq!(parse_resistance("abc"), None);
    }

    #[test]
    fn alt_capacitance() {
        approx(parse_capacitance("100pF").unwrap(), 100e-12);
        approx(parse_capacitance("10p").unwrap(), 10e-12);
        approx(parse_capacitance("100nF").unwrap(), 100e-9);
        approx(parse_capacitance("10n").unwrap(), 10e-9);
        approx(parse_capacitance("10uF").unwrap(), 10e-6);
        approx(parse_capacitance("10µF").unwrap(), 10e-6);
        approx(parse_capacitance("100u").unwrap(), 100e-6);
        approx(parse_capacitance("1mF").unwrap(), 1e-3);
        assert_eq!(parse_capacitance(""), None);
    }

    #[test]
    fn alt_inductance() {
        approx(parse_inductance("100nH").unwrap(), 100e-9);
        approx(parse_inductance("10uH").unwrap(), 10e-6);
        approx(parse_inductance("10µH").unwrap(), 10e-6);
        approx(parse_inductance("1mH").unwrap(), 1e-3);
        assert_eq!(parse_inductance(""), None);
    }

    #[test]
    fn alt_frequency() {
        assert_eq!(parse_frequency("100Hz"), Some(100.0));
        assert_eq!(parse_frequency("100"), Some(100.0));
        approx(parse_frequency("32.768kHz").unwrap(), 32768.0);
        approx(parse_frequency("100KHz").unwrap(), 100000.0);
        approx(parse_frequency("8MHz").unwrap(), 8e6);
        approx(parse_frequency("16MHz").unwrap(), 16e6);
        approx(parse_frequency("2.4GHz").unwrap(), 2.4e9);
        assert_eq!(parse_frequency(""), None);
    }

    #[test]
    fn alt_decibels() {
        assert_eq!(parse_decibels("85dB"), Some(85.0));
        assert_eq!(parse_decibels("90 dB"), Some(90.0));
        assert_eq!(parse_decibels("75.5dB"), Some(75.5));
        assert_eq!(parse_decibels(""), None);
        assert_eq!(parse_decibels("loud"), None);
    }

    #[test]
    fn alt_parser_edge_cases() {
        assert_eq!(parse_resistance("0"), Some(0.0));
        assert_eq!(parse_resistance("0Ω"), Some(0.0));
        assert_eq!(parse_resistance("10kΩ ±1%"), Some(10000.0));
        approx(parse_capacitance("100nF ±5%").unwrap(), 100e-9);
        assert_eq!(parse_voltage("0V"), Some(0.0));
        assert_eq!(parse_current("0A"), Some(0.0));
        assert_eq!(parse_power("0W"), Some(0.0));
    }

    #[test]
    fn alt_impedance_at_freq() {
        let r = parse_impedance_at_freq("600Ω @ 100MHz").unwrap();
        approx(r.0, 600.0);
        approx(r.1, 100e6);
        let r = parse_impedance_at_freq("1kOhm @ 100MHz").unwrap();
        approx(r.0, 1000.0);
        approx(r.1, 100e6);
        assert_eq!(parse_impedance_at_freq(""), None);
        assert_eq!(parse_impedance_at_freq("just some text"), None);
    }

    #[test]
    fn alt_impedance_at_freq_match() {
        assert!(impedance_at_freq_match("600Ω @ 100MHz", "600Ω @ 100MHz"));
        assert!(impedance_at_freq_match("600Ω @ 100MHz", "606Ω @ 100MHz"));
        assert!(!impedance_at_freq_match("600Ω @ 100MHz", "1200Ω @ 100MHz"));
        assert!(!impedance_at_freq_match("600Ω @ 100MHz", "600Ω @ 200MHz"));
    }

    #[test]
    fn parse_integer_overflow_no_panic() {
        // Normal cases still work
        assert_eq!(parse_integer("42"), Some(42));
        assert_eq!(parse_integer("0"), Some(0));

        // Overflow returns None instead of panicking (this was previously a panic)
        assert_eq!(parse_integer("99999999999999999999"), None);
        assert_eq!(parse_integer("9223372036854775808"), None); // i64::MAX + 1

        // Edge cases at i64 boundaries
        assert_eq!(parse_integer("9223372036854775807"), Some(9223372036854775807)); // i64::MAX
    }
}
