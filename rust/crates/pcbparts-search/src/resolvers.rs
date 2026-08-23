use pcbparts_parsers::manufacturer_aliases::{known_manufacturers, manufacturer_aliases};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

struct SynonymGroup {
    primary: &'static str,
    patterns: Vec<Regex>,
}

static SYNONYM_GROUPS: LazyLock<Vec<SynonymGroup>> = LazyLock::new(|| {
    vec![SynonymGroup {
        primary: "IPEX",
        patterns: vec![
            Regex::new(r"(?i)u\.fl").unwrap(),
            Regex::new(r"(?i)mhf").unwrap(),
            Regex::new(r"(?i)i-pex").unwrap(),
            Regex::new(r"(?i)hirose u\.fl").unwrap(),
            Regex::new(r"(?i)ipx").unwrap(),
        ],
    }]
});

/// Expand query with synonyms for better search results (e.g. "U.FL" -> "IPEX").
pub fn expand_query_synonyms(query: &str) -> String {
    let mut result = query.to_string();
    for group in SYNONYM_GROUPS.iter() {
        for pattern in &group.patterns {
            if pattern.is_match(&result) {
                result = pattern.replace_all(&result, group.primary).to_string();
                break;
            }
        }
    }
    result
}

pub fn package_families() -> HashMap<&'static str, Vec<&'static str>> {
    HashMap::from([
        ("0402", vec!["0402", "1005"]),
        ("0603", vec!["0603", "1608"]),
        ("0805", vec!["0805", "2012"]),
        ("1206", vec!["1206", "3216"]),
        ("sot-23", vec!["SOT-23", "SOT-23-3", "SOT-23-3L", "SOT-23(TO-236)"]),
        ("sot-23-5", vec!["SOT-23-5", "SOT-23-5L"]),
        ("sot-23-6", vec!["SOT-23-6", "SOT-23-6L"]),
        ("sot-223", vec!["SOT-223", "SOT-223-3", "SOT-223-3L", "SOT-223-4"]),
        ("sot-89", vec!["SOT-89", "SOT-89-3", "SOT-89-3L"]),
        ("to-252", vec!["TO-252", "TO-252-2", "TO-252-2L", "DPAK"]),
        ("to-263", vec!["TO-263", "TO-263-2", "D2PAK"]),
        ("to-220", vec!["TO-220", "TO-220-3", "TO-220F", "TO-220F-3"]),
        ("qfn-16", vec!["QFN-16", "QFN-16-EP(3x3)", "QFN-16-EP(4x4)", "QFN-16(3x3)", "VQFN-16"]),
        ("qfn-24", vec!["QFN-24", "QFN-24-EP(4x4)", "VQFN-24", "VQFN-24-EP(4x4)"]),
        ("qfn-32", vec!["QFN-32", "QFN-32-EP(5x5)", "VQFN-32", "VQFN-32-EP(5x5)"]),
        ("so-8", vec!["SO-8", "SOP-8", "SOIC-8"]),
        ("sop-8", vec!["SO-8", "SOP-8", "SOIC-8"]),
        ("soic-8", vec!["SO-8", "SOP-8", "SOIC-8"]),
        ("so8", vec!["SO-8", "SOP-8", "SOIC-8"]),
        ("sop8", vec!["SO-8", "SOP-8", "SOIC-8"]),
        ("soic8", vec!["SO-8", "SOP-8", "SOIC-8"]),
        ("so-16", vec!["SO-16", "SOP-16", "SOIC-16"]),
        ("sop-16", vec!["SO-16", "SOP-16", "SOIC-16"]),
        ("soic-16", vec!["SO-16", "SOP-16", "SOIC-16"]),
    ])
}

pub fn imperial_chip_sizes() -> HashSet<&'static str> {
    HashSet::from([
        "01005", "0201", "03015", "0402", "0603", "0612", "0805", "0806",
        "1008", "1206", "1210", "1212", "1218", "1806", "1808", "1812",
        "2010", "2220", "2410", "2512", "2920", "3920", "5930",
    ])
}

pub fn smd_package_families() -> HashMap<&'static str, Vec<&'static str>> {
    HashMap::from([
        ("1610", vec!["SMD1610", "SMD1610-2P"]),
        ("1612", vec!["SMD1612-4P"]),
        ("2012", vec!["SMD2012-2P", "SMD2012-4P", "SMD2012-8P"]),
        ("2016", vec!["SMD2016", "SMD2016-2P", "SMD2016-4P", "SMD2016-6P"]),
        ("2520", vec!["SMD2520", "SMD2520-2P", "SMD2520-4P", "SMD2520-6P"]),
        ("2835", vec!["SMD2835", "SMD2835-2P", "SMD2835-3P", "SMD2835-4P", "SMD2835-6P"]),
        ("3014", vec!["SMD3014-2P"]),
        ("3020", vec!["SMD3020", "SMD3020-3P"]),
        ("3030", vec!["SMD3030", "SMD3030-2P", "SMD3030-3P", "SMD3030-4P", "SMD3030-6P", "SMD3030-7P"]),
        ("3215", vec!["SMD3215", "SMD3215-2P", "SMD3215-4P", "SMD3215-8P"]),
        ("3225", vec!["SMD3225", "SMD3225-2P", "SMD3225-4P", "SMD3225-6P", "SMD3225-10P", "SMD3225-14P", "SMD-3225_4P"]),
        ("3528", vec!["SMD3528", "SMD3528-2P", "SMD3528-3P", "SMD3528-4P", "SMD3528-6P"]),
        ("3535", vec!["SMD3535", "SMD3535-2P", "SMD3535-3P", "SMD3535-4P", "SMD3535-5P", "SMD3535-6P"]),
        ("5032", vec!["SMD5032", "SMD5032-2P", "SMD5032-4P", "SMD5032-6P", "SMD-5032-4P"]),
        ("5050", vec!["SMD5050", "SMD5050-2P", "SMD5050-4P", "SMD5050-6P", "SMD5050-8P"]),
        ("5730", vec!["SMD5730", "SMD5730-3P"]),
        ("6035", vec!["SMD6035-2P", "SMD6035-4P"]),
        ("7050", vec!["SMD7050", "SMD7050-2P", "SMD7050-4P", "SMD7050-6P", "SMD7050-10P"]),
        ("7060", vec!["SMD7060", "SMD7060-2P", "SMD7060-3P"]),
        ("8045", vec!["SMD8045-2P"]),
        ("8080", vec!["SMD8080-2P", "SMD8080-3P", "SMD8080-4P", "SMD8080-5P", "SMD8080-6P"]),
        ("9070", vec!["SMD9070-8P"]),
    ])
}

static BARE_DIMENSION_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{4}$").unwrap());
static SMD_PREFIX_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^smd-?(\d{4,5})(?:-\d+p)?$").unwrap());

/// Expand a package name to include family variants.
pub fn expand_package(package: &str) -> Vec<String> {
    let pkg_lower = package.to_lowercase();

    if let Some(variants) = package_families().get(pkg_lower.as_str()) {
        return variants.iter().map(|s| s.to_string()).collect();
    }

    if BARE_DIMENSION_RE.is_match(package) && !imperial_chip_sizes().contains(package) {
        if let Some(variants) = smd_package_families().get(package) {
            return variants.iter().map(|s| s.to_string()).collect();
        }
    }

    if let Some(caps) = SMD_PREFIX_RE.captures(&pkg_lower) {
        let dim = &caps[1];
        if let Some(variants) = smd_package_families().get(dim) {
            return variants.iter().map(|s| s.to_string()).collect();
        }
    }

    vec![package.to_string()]
}

fn manufacturer_lower_to_exact() -> HashMap<String, &'static str> {
    known_manufacturers().into_iter().map(|name| (name.to_lowercase(), name)).collect()
}

/// Resolve manufacturer alias to canonical name.
pub fn resolve_manufacturer(name: &str) -> String {
    let name_lower = name.to_lowercase();

    if let Some(&canonical) = manufacturer_aliases().get(name_lower.as_str()) {
        return canonical.to_string();
    }

    if let Some(&exact) = manufacturer_lower_to_exact().get(&name_lower) {
        return exact.to_string();
    }

    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_query_synonyms() {
        assert_eq!(expand_query_synonyms("U.FL connector"), "IPEX connector");
        assert_eq!(expand_query_synonyms("i-pex 4pin"), "IPEX 4pin");
        assert_eq!(expand_query_synonyms("MHF connector"), "IPEX connector");
        assert_eq!(expand_query_synonyms("no match here"), "no match here");
        assert_eq!(expand_query_synonyms("IPX"), "IPEX");
    }

    #[test]
    fn test_expand_package_family() {
        assert_eq!(
            expand_package("SOT-23"),
            vec!["SOT-23", "SOT-23-3", "SOT-23-3L", "SOT-23(TO-236)"]
        );
        assert_eq!(expand_package("0603"), vec!["0603", "1608"]);
    }

    #[test]
    fn test_expand_package_smd_bare_dimension() {
        assert_eq!(
            expand_package("3215"),
            vec!["SMD3215", "SMD3215-2P", "SMD3215-4P", "SMD3215-8P"]
        );
    }

    #[test]
    fn test_expand_package_smd_prefix() {
        assert_eq!(
            expand_package("SMD3215"),
            vec!["SMD3215", "SMD3215-2P", "SMD3215-4P", "SMD3215-8P"]
        );
        assert_eq!(
            expand_package("smd-3215-2p"),
            vec!["SMD3215", "SMD3215-2P", "SMD3215-4P", "SMD3215-8P"]
        );
    }

    #[test]
    fn test_expand_package_no_expansion() {
        assert_eq!(expand_package("QFN-24-EP(4x4)"), vec!["QFN-24-EP(4x4)"]);
        assert_eq!(expand_package("unknown-pkg"), vec!["unknown-pkg"]);
    }

    #[test]
    fn test_resolve_manufacturer_alias() {
        assert_eq!(resolve_manufacturer("TI"), "Texas Instruments");
        assert_eq!(resolve_manufacturer("texas instruments"), "Texas Instruments");
    }

    #[test]
    fn test_resolve_manufacturer_known_case_insensitive() {
        assert_eq!(resolve_manufacturer("YAGEO"), "YAGEO");
        assert_eq!(resolve_manufacturer("yageo"), "YAGEO");
    }

    #[test]
    fn test_resolve_manufacturer_unknown_passthrough() {
        assert_eq!(resolve_manufacturer("Totally Unknown Co"), "Totally Unknown Co");
    }

    #[test]
    fn test_package_families_count() {
        // 4 imperial + 3 sot23-variants + 1 sot223 + 1 sot89 + 3 to-packages
        // + 3 qfn + 9 so/sop/soic aliases = 24 keys total
        assert_eq!(package_families().len(), 24);
    }

    #[test]
    fn test_imperial_chip_sizes_excludes_from_smd_expansion() {
        // "0402" is an imperial chip size, so even though it's a bare 4-digit
        // string it must NOT be looked up in SMD_PACKAGE_FAMILIES.
        assert!(imperial_chip_sizes().contains("0402"));
        assert_eq!(expand_package("0402"), vec!["0402", "1005"]); // handled by PACKAGE_FAMILIES instead
    }
}
