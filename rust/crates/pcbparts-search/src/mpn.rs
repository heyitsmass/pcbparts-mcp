use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

pub const MPN_TRAILING_SUFFIXES: &[&str] = &[
    "-TR", "/TR", "-T", "-CT", "-ND", "-DK", "#PBF", "-PBF", "#PBFREE", "-PBFREE", "+T", "+TR",
];

static MPN_INSERT_T_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^([A-Z]{2,5}\d{2,5})(-[A-Z0-9/]+)$").unwrap());

/// Generate normalized variants of an MPN query for better matching.
///
/// Returns variants in order of preference: original query first, then
/// trailing-suffix-stripped, then "T"-inserted (tape & reel) variants —
/// deduplicated case-insensitively.
pub fn normalize_mpn(query: &str) -> Vec<String> {
    let mut variants = vec![query.to_string()];
    let mut seen_upper: HashSet<String> = HashSet::new();
    seen_upper.insert(query.to_uppercase());
    let working = query.to_uppercase();

    let mut stripped = working.clone();
    for suffix in MPN_TRAILING_SUFFIXES {
        if stripped.ends_with(suffix) {
            stripped.truncate(stripped.len() - suffix.len());
            break;
        }
    }

    if !seen_upper.contains(&stripped.to_uppercase()) {
        variants.push(stripped.clone());
        seen_upper.insert(stripped.to_uppercase());
    }

    for candidate in [&working, &stripped] {
        if let Some(caps) = MPN_INSERT_T_PATTERN.captures(candidate) {
            let base = &caps[1];
            let suffix = &caps[2];
            if !base.ends_with('T') {
                let with_t = format!("{base}T{suffix}");
                if !seen_upper.contains(&with_t.to_uppercase()) {
                    seen_upper.insert(with_t.to_uppercase());
                    variants.push(with_t);
                }
            }
        }
    }

    variants
}

/// Check if a query looks like a manufacturer part number.
pub fn looks_like_mpn(query: &str) -> bool {
    let char_count = query.chars().count();
    if query.is_empty() || char_count < 4 || char_count > 40 {
        return false;
    }

    let has_letter = query.chars().any(|c| c.is_alphabetic());
    let has_digit = query.chars().any(|c| c.is_ascii_digit());
    if !(has_letter && has_digit) {
        return false;
    }

    static IC_STYLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^[A-Z]{1,5}\d{2,}").unwrap());
    if IC_STYLE.is_match(query) {
        return true;
    }

    static DIODE_STYLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\d[A-Z]\d{3,}").unwrap());
    if DIODE_STYLE.is_match(query) {
        return true;
    }

    query.contains('-') || query.contains('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- TestLooksLikeMpn ---
    #[test]
    fn test_typical_ic_mpn() {
        assert!(looks_like_mpn("STM32F103C8T6"));
        assert!(looks_like_mpn("MCP73831-2ACI/MC"));
        assert!(looks_like_mpn("ESP32-C3"));
    }
    #[test]
    fn test_with_suffixes() {
        assert!(looks_like_mpn("STM32F103C8T6-TR"));
        assert!(looks_like_mpn("LM1117-3.3#PBF"));
    }
    #[test]
    fn test_short_mpn() {
        assert!(looks_like_mpn("NE555"));
        assert!(looks_like_mpn("1N4148"));
        assert!(looks_like_mpn("2N2222"));
    }
    #[test]
    fn test_not_mpn() {
        assert!(!looks_like_mpn("resistor"));
        assert!(!looks_like_mpn("10k"));
        assert!(!looks_like_mpn(""));
        assert!(!looks_like_mpn("abc"));
    }
    #[test]
    fn test_case_insensitive() {
        assert!(looks_like_mpn("stm32f103c8t6"));
        assert!(looks_like_mpn("Stm32F103c8T6"));
        assert!(looks_like_mpn("mcp73831-2aci/mc"));
    }

    // --- TestNormalizeMpn ---
    #[test]
    fn test_no_change_needed() {
        let result = normalize_mpn("LM1117-3.3");
        assert_eq!(result[0], "LM1117-3.3");
        assert_eq!(result.len(), 1);
    }
    #[test]
    fn test_strip_tr_suffix() {
        let result = normalize_mpn("STM32F103C8T6-TR");
        assert!(result.contains(&"STM32F103C8T6-TR".to_string()));
        assert!(result.contains(&"STM32F103C8T6".to_string()));
    }
    #[test]
    fn test_strip_pbf_suffix() {
        let result = normalize_mpn("LM1117-3.3#PBF");
        assert!(result.contains(&"LM1117-3.3#PBF".to_string()));
        assert!(result.contains(&"LM1117-3.3".to_string()));
    }
    #[test]
    fn test_insert_t_for_tape_reel() {
        let result = normalize_mpn("MCP73831-2ACI/MC");
        assert!(result.contains(&"MCP73831-2ACI/MC".to_string()));
        assert!(result.contains(&"MCP73831T-2ACI/MC".to_string()));
    }
    #[test]
    fn test_already_has_t() {
        let result = normalize_mpn("MCP73831T-2ACI/MC");
        assert!(!result.contains(&"MCP73831TT-2ACI/MC".to_string()));
    }
    #[test]
    fn test_original_always_first() {
        let result = normalize_mpn("MCP73831-2ACI/MC");
        assert_eq!(result[0], "MCP73831-2ACI/MC");
    }
    #[test]
    fn test_combined_strip_and_insert() {
        let result = normalize_mpn("MCP73831-2ACI-TR");
        assert!(result.contains(&"MCP73831-2ACI-TR".to_string()));
        assert!(result.contains(&"MCP73831-2ACI".to_string()));
        assert!(result.contains(&"MCP73831T-2ACI".to_string()));
    }
    #[test]
    fn test_lowercase_input() {
        let result = normalize_mpn("stm32f103c8t6-tr");
        assert_eq!(result[0], "stm32f103c8t6-tr");
        assert!(result.iter().any(|v| v.to_uppercase().contains("STM32F103C8T6")));
    }
    #[test]
    fn test_mixed_case_input() {
        let result = normalize_mpn("Stm32F103C8T6-TR");
        assert_eq!(result[0], "Stm32F103C8T6-TR");
        assert!(result.len() >= 2);
    }
    #[test]
    fn test_no_duplicate_variants() {
        let result = normalize_mpn("stm32f103c8t6-tr");
        let mut seen_upper = std::collections::HashSet::new();
        for v in &result {
            let v_upper = v.to_uppercase();
            assert!(!seen_upper.contains(&v_upper), "Duplicate variant: {v}");
            seen_upper.insert(v_upper);
        }
    }
}
