use std::collections::HashMap;

fn fts_stop_words() -> &'static [&'static str] {
    &[
        "a", "an", "the", "and", "or", "of", "for", "with", "in", "on", "to", "is", "it", "by",
        "at", "from", "as", "be", "my", "me", "do", "no", "so", "up", "if", "am", "are", "was",
        "has", "have", "had", "not", "but", "can", "will", "would", "could", "should", "what",
        "which", "that", "this", "these", "those", "how", "when", "where", "who", "than", "then",
        "also", "just", "very", "really", "any", "some", "about", "like", "into", "over", "such",
        "sensor", "sensors", "module", "modules", "board", "chip", "ic", "breakout", "give",
        "find", "show", "get", "list", "all", "best", "good", "recommend", "need", "want",
        "looking", "search", "use", "using", "used", "make", "work", "works", "detect",
        "measure", "monitor", "read", "reading",
    ]
}

pub fn sanitize_fts_query(query: &str) -> String {
    let clean = query.replace('"', "").replace('\'', "");
    let stop_words = fts_stop_words();
    let quoted: Vec<String> = clean
        .split_whitespace()
        .filter(|t| {
            let lower = t.to_lowercase();
            t.chars().count() >= 2 && !stop_words.contains(&lower.as_str())
        })
        .map(|t| format!("\"{}\"*", t))
        .collect();
    quoted.join(" AND ")
}

fn measure_expansions() -> HashMap<&'static str, Vec<&'static str>> {
    HashMap::from([("imu", vec!["acceleration", "gyroscope", "magnetic_field"])])
}

fn measure_query_aliases() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("barometric", "pressure"),
        ("altimeter", "pressure"),
        ("barometer", "pressure"),
        ("range finder", "distance"),
        ("rangefinder", "distance"),
        ("encoder", "rotation"),
        ("carbon monoxide", "co"),
        ("compass", "magnetic_field"),
        ("magnetometer", "magnetic_field"),
        ("accelerometer", "acceleration"),
        ("gyro", "gyroscope"),
        ("lux", "light"),
        ("ambient light", "light"),
        ("thermometer", "temperature"),
        ("hygrometer", "humidity"),
        ("air quality", "gas"),
        ("dust", "particulate"),
        ("pm2.5", "particulate"),
        ("pm10", "particulate"),
        ("sonar", "ultrasonic"),
    ])
}

pub fn protocol_aliases() -> HashMap<&'static str, Vec<&'static str>> {
    HashMap::from([("gpio", vec!["analog", "digital", "pwm", "one_wire"])])
}

#[derive(Debug, PartialEq)]
pub enum MeasureMode {
    Or,
    Single,
}

/// Resolve a single measure string to actual measure values and mode.
pub fn resolve_measure(measure: &str) -> (Vec<String>, MeasureMode) {
    let lower = measure.to_lowercase();
    let lower = lower.trim();

    if let Some(expansion) = measure_expansions().get(lower) {
        return (
            expansion.iter().map(|s| s.to_string()).collect(),
            MeasureMode::Or,
        );
    }
    if let Some(alias) = measure_query_aliases().get(lower) {
        return (vec![alias.to_string()], MeasureMode::Single);
    }
    (vec![lower.to_string()], MeasureMode::Single)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- sanitize_fts_query ---
    #[test]
    fn test_basic_query() {
        assert_eq!(sanitize_fts_query("BME280"), "\"BME280\"*");
    }
    #[test]
    fn test_multi_term() {
        assert_eq!(sanitize_fts_query("temperature humidity"), "\"temperature\"* AND \"humidity\"*");
    }
    #[test]
    fn test_stop_words_filtered() {
        assert_eq!(sanitize_fts_query("temperature and humidity"), "\"temperature\"* AND \"humidity\"*");
        assert_eq!(sanitize_fts_query("give me all temperature sensors"), "\"temperature\"*");
    }
    #[test]
    fn test_strips_quotes() {
        assert_eq!(sanitize_fts_query("\"test\""), "\"test\"*");
    }
    #[test]
    fn test_skips_short_terms() {
        assert_eq!(sanitize_fts_query("a temperature"), "\"temperature\"*");
    }
    #[test]
    fn test_empty() {
        assert_eq!(sanitize_fts_query(""), "");
    }
    #[test]
    fn test_prefix_match_format() {
        assert_eq!(sanitize_fts_query("BM22S"), "\"BM22S\"*");
    }

    // --- resolve_measure ---
    #[test]
    fn test_imu_expansion() {
        let (measures, mode) = resolve_measure("imu");
        assert_eq!(mode, MeasureMode::Or);
        let set: std::collections::HashSet<_> = measures.into_iter().collect();
        assert_eq!(set, ["acceleration", "gyroscope", "magnetic_field"].iter().map(|s| s.to_string()).collect());
    }
    #[test]
    fn test_voc_passthrough() {
        let (measures, mode) = resolve_measure("voc");
        assert_eq!(mode, MeasureMode::Single);
        assert_eq!(measures, vec!["voc".to_string()]);
    }
    #[test]
    fn test_alias_barometric() {
        let (measures, _) = resolve_measure("barometric");
        assert_eq!(measures, vec!["pressure".to_string()]);
    }
    #[test]
    fn test_passthrough() {
        let (measures, mode) = resolve_measure("co2");
        assert_eq!(mode, MeasureMode::Single);
        assert_eq!(measures, vec!["co2".to_string()]);
    }
    #[test]
    fn test_case_insensitive() {
        let (measures, _) = resolve_measure("IMU");
        let set: std::collections::HashSet<_> = measures.into_iter().collect();
        assert_eq!(set, ["acceleration", "gyroscope", "magnetic_field"].iter().map(|s| s.to_string()).collect());
    }
}
