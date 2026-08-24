use regex::Regex;
use std::sync::LazyLock;

static MODEL_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // ESP32 compound module names must come first so the full compound name
        // (e.g. "ESP32-S3-MINI-1") matches before the generic pattern below truncates it.
        Regex::new(r"(?i)\b(ESP32(?:-[A-Z0-9]+)+)\b").unwrap(),
        Regex::new(r"(?i)\b(STM32[A-Z]\d+[A-Z0-9]*|RP2040|ATMEGA\d+[A-Z]*|PIC\d+[A-Z0-9]*)\b").unwrap(),
        Regex::new(r"(?i)\b(TP[45]\d{3}|AMS\d{4}|LM\d{4}|NE555|TL\d{3}|LMV?\d{3,4}|TPS\d{4,5})\b").unwrap(),
        Regex::new(r"(?i)\b(AO\d{4}|SI\d{4}|IRF\d{3,4}|IRLZ?\d{2,4}|2N\d{4}|BC\d{3})\b").unwrap(),
        Regex::new(r"(?i)\b(WS2812[A-Z]*|SK6812|APA102|TLC5940)\b").unwrap(),
        Regex::new(r"(?i)\b(1N\d{4}[A-Z]*|1SS\d{3}[A-Z]*|BAT\d{2}[A-Z]*|BAS\d{2}[A-Z]*|BAV\d{2}[A-Z]*)\b").unwrap(),
        // Generic IC model numbers (last resort) — intentionally last to avoid
        // truncating the more specific patterns above.
        Regex::new(r"(?i)\b([A-Z]{2,5}\d{2,5}[A-Z]?\d*(?:-[A-Z0-9]+)?)\b").unwrap(),
    ]
});

const PACKAGE_LIKE_PATTERNS: &[&str] = &[
    "SOT", "SOD", "SOP", "SOIC", "SSOP", "TSSOP", "MSOP", "QSOP",
    "QFN", "DFN", "QFP", "LQFP", "TQFP", "BGA", "DIP", "SIP",
    "CSP", "WLCSP", "LFCSP", "UCSP", "VCSP",
];

const AMBIGUOUS_WORDS: &[&str] = &[
    "LED", "LCD", "USB", "SPI", "I2C", "ADC", "DAC", "MCU", "CPU", "GPU",
    "RJ45", "RJ11", "RJ12", "RJ9", "RJ22", "RJ25",
];

/// Extract a likely model number from `query`. Returns `(model, remaining_query)`.
pub fn extract_model_number(query: &str) -> (Option<String>, String) {
    for pattern in MODEL_PATTERNS.iter() {
        let Some(caps) = pattern.captures(query) else { continue };
        let whole = caps.get(0).unwrap();
        let model = caps.get(1).unwrap().as_str();
        let model_upper = model.to_uppercase();

        if AMBIGUOUS_WORDS.contains(&model_upper.as_str()) {
            continue;
        }

        let mut is_package = false;
        for pkg_prefix in PACKAGE_LIKE_PATTERNS {
            if model_upper.starts_with(pkg_prefix) && model_upper.len() > pkg_prefix.len() {
                let rest = &model_upper[pkg_prefix.len()..];
                let rest_is_digits = !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit());
                let rest_is_digits_then_l = rest.chars().next().is_some_and(|c| c.is_ascii_digit())
                    && rest.chars().all(|c| c.is_ascii_digit() || c == 'L');
                if rest_is_digits || rest_is_digits_then_l {
                    is_package = true;
                    break;
                }
            }
        }
        if is_package {
            continue;
        }

        let remaining = format!("{}{}", &query[..whole.start()], &query[whole.end()..]);
        return (Some(model.to_string()), remaining.trim().to_string());
    }

    (None, query.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_extraction() {
        for (query, expected_model) in [
            ("ESP32-S3-MINI-1", "ESP32-S3-MINI-1"),
            ("ESP32-S3-MINI", "ESP32-S3-MINI"),
            ("ESP32-C3-MINI", "ESP32-C3-MINI"),
            ("ESP32-S3-WROOM-1", "ESP32-S3-WROOM-1"),
            ("ESP32-S3-MINI-1-N8", "ESP32-S3-MINI-1-N8"),
            ("ESP32-C3", "ESP32-C3"),
            ("ESP32-S3", "ESP32-S3"),
            ("STM32F103C8T6", "STM32F103C8T6"),
            ("RP2040", "RP2040"),
            ("ATMEGA328P", "ATMEGA328P"),
            ("TP4056", "TP4056"),
            ("AMS1117", "AMS1117"),
            ("NE555", "NE555"),
        ] {
            let (model, _remaining) = extract_model_number(query);
            let model = model.unwrap_or_else(|| panic!("should extract model from '{query}'"));
            assert_eq!(model.to_uppercase(), expected_model.to_uppercase());
        }
    }

    #[test]
    fn esp32_mini_not_truncated() {
        let (model, remaining) = extract_model_number("ESP32-S3-MINI-1 module");
        assert_eq!(model, Some("ESP32-S3-MINI-1".to_string()));
        assert_eq!(remaining.trim(), "module");
    }

    #[test]
    fn package_not_detected_as_model() {
        const PACKAGE_LIKE: [&str; 8] =
            ["SOT", "SOD", "SOP", "SOIC", "QFN", "DFN", "TSSOP", "DIP"];
        for query in ["NPN SOT23", "diode SOD323", "driver QFN32", "ic TSSOP20", "mosfet SOIC8", "amp DIP8"] {
            let (model, _remaining) = extract_model_number(query);
            if let Some(model) = model {
                let model_upper = model.to_uppercase();
                for prefix in PACKAGE_LIKE {
                    let looks_like_package = model_upper.starts_with(prefix)
                        && model_upper.len() > prefix.len()
                        && model_upper[prefix.len()..].chars().all(|c| c.is_ascii_digit() || c == 'L');
                    assert!(!looks_like_package, "'{model}' looks like a package and should not be a model number");
                }
            }
        }
    }
}
