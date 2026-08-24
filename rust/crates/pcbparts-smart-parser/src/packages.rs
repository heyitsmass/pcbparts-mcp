use regex::Regex;
use std::sync::LazyLock;

static IMPERIAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(01005|0201|0402|0603|0805|1206|1210|1812|2010|2512)\b").unwrap());
static SMD_METRIC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(1610|1612|2012|2016|2520|2835|3014|3020|3030|3215|3225|3528|3535|5032|5050|5730|6035|7050|7060|8045|8080|9070)\b").unwrap()
});
static METRIC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(0402M|0603M|0805M|1206M)\b").unwrap());
static SOT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(SOT-?23(?:-\d+)?L?|SOT-?89(?:-\d+)?|SOT-?223(?:-\d+)?|SOT-?323(?:-\d+)?|SOT-?363(?:-\d+)?|SOT-?523(?:-\d+)?|SOT-?723(?:-\d+)?)\b").unwrap()
});
static SOD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(SOD-?(?:123|323|523|923|128|882|80|110|123FL|323FL))\b").unwrap()
});
static DO_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(DO-?(?:35|41|201|204|214|215|218|219|220)(?:AA|AB|AC|AD|AE|AF|AG)?)\b").unwrap()
});
static TO_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(TO-?92(?:S|L)?|TO-?220(?:F|FP|AB)?(?:-\d+)?|TO-?252(?:-\d+)?|TO-?263(?:-\d+)?|TO-?247(?:-\d+)?|TO-?251|TO-?3P(?:F)?|DPAK|D2PAK|D3PAK)\b").unwrap()
});
static QFN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b((?:V)?QFN-?\d+(?:-EP)?(?:\([^)]+\))?|DFN-?\d+(?:-EP)?(?:\([^)]+\))?|WQFN-?\d+|TQFN-?\d+|UQFN-?\d+)\b").unwrap()
});
static QFP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b((?:L|T|H|PQ)?QFP-?\d+(?:\([^)]+\))?)\b").unwrap());
static BGA_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b((?:FC|W|T|M|U|P|F)?BGA-?\d+(?:\([^)]+\))?)\b").unwrap());
static CSP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b((?:WL|LF|U|FC|V)?CSP-?\d+(?:-EP)?(?:\([^)]+\))?)\b").unwrap()
});
static DIP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b((?:P|S|SK|C)?DIP-?\d+(?:\([^)]+\))?|SIP-?\d+)\b").unwrap());
static TSSOP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(TSSOP-?\d+|SSOP-?\d+|MSOP-?\d+|QSOP-?\d+|HTSSOP-?\d+|VSSOP-?\d+)\b").unwrap()
});
static SOP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(SOP-?\d+(?:-\d+)?(?:\([^)]+\))?|SOIC-?\d+(?:-\d+)?(?:\([^)]+\))?)\b").unwrap()
});
static SO_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(SO-?\d+)\b").unwrap());
static MODULE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(SMD-?\d+|LGA-?\d+)\b").unwrap());
// Python: r'\b(SM[ABC])\b(?!\s*connector)' — the trailing negative lookahead has no
// `regex`-crate equivalent. Matched unconstrained here via DIODE_PKG_RE and filtered
// candidate-by-candidate in `find_diode_pkg`, which reproduces `re.search`'s
// leftmost-match-satisfying-the-whole-pattern behavior exactly.
static DIODE_PKG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(SM[ABC])\b").unwrap());
static FOLLOWED_BY_CONNECTOR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*connector").unwrap());
static MXX_DIODE_PKG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(M[478])\b").unwrap());
static USB_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(USB-?[ABC]|TYPE-?[ABC]|MICRO-?USB|MINI-?USB)\b").unwrap());

static SOT_HYPHEN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"SOT(\d)").unwrap());
static SOD_HYPHEN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"SOD(\d)").unwrap());
static TO_HYPHEN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"TO(\d)").unwrap());

fn find_diode_pkg(query: &str) -> Option<regex::Match<'_>> {
    DIODE_PKG_RE.find_iter(query).find(|m| !FOLLOWED_BY_CONNECTOR_RE.is_match(&query[m.end()..]))
}

/// A package-matcher entry: a finder function plus its `kind` label. Aliased to silence
/// `clippy::type_complexity` on the `PACKAGE_PATTERNS` array type below.
type PackagePatternFinder = fn(&str) -> Option<regex::Match<'_>>;
type PackagePatternEntry = (PackagePatternFinder, &'static str);

/// Precompiled package-matcher table, in match-priority order (order is load-bearing —
/// e.g. TSSOP must be checked before SOP, and SOP before SO). Rust equivalent of
/// Python's `PACKAGE_PATTERNS: list[tuple[re.Pattern[str], str]]` module-level list,
/// re-exported at the crate root for `__all__` parity (final-review-report.md finding
/// #4). Every pattern has exactly one capturing group whose span equals the whole match
/// (each regex is `\b(...)\b` with nothing captured outside the group), so `m.as_str()`
/// on the whole-match `Match` is always the captured package text — no separate
/// `.captures()` call is needed.
pub static PACKAGE_PATTERNS: [PackagePatternEntry; 19] = [
    (|q| IMPERIAL_RE.find(q), "imperial"),
    (|q| SMD_METRIC_RE.find(q), "smd_metric"),
    (|q| METRIC_RE.find(q), "metric"),
    (|q| SOT_RE.find(q), "sot"),
    (|q| SOD_RE.find(q), "sod"),
    (|q| DO_RE.find(q), "do"),
    (|q| TO_RE.find(q), "to"),
    (|q| QFN_RE.find(q), "qfn"),
    (|q| QFP_RE.find(q), "qfp"),
    (|q| BGA_RE.find(q), "bga"),
    (|q| CSP_RE.find(q), "csp"),
    (|q| DIP_RE.find(q), "dip"),
    (|q| TSSOP_RE.find(q), "tssop"),
    (|q| SOP_RE.find(q), "sop"),
    (|q| SO_RE.find(q), "so"),
    (|q| MODULE_RE.find(q), "module"),
    (find_diode_pkg, "diode_pkg"),
    (|q| MXX_DIODE_PKG_RE.find(q), "mxx_diode_pkg"),
    (|q| USB_RE.find(q), "usb"),
];

/// Extract package from `query`. Returns `(package, remaining_query,
/// suggested_subcategory)` — `suggested_subcategory` is used for USB-C etc. where the
/// package pattern implies a component type rather than a literal package name.
pub fn extract_package(query: &str) -> (Option<String>, String, Option<String>) {
    for (find_fn, kind) in PACKAGE_PATTERNS {
        if let Some(m) = find_fn(query) {
            return finish_package_match(query, m, kind);
        }
    }
    (None, query.to_string(), None)
}

fn finish_package_match(query: &str, m: regex::Match<'_>, kind: &str) -> (Option<String>, String, Option<String>) {
    let mut package = m.as_str().to_uppercase();
    package = SOT_HYPHEN_RE.replace_all(&package, "SOT-$1").to_string();
    package = SOD_HYPHEN_RE.replace_all(&package, "SOD-$1").to_string();
    package = TO_HYPHEN_RE.replace_all(&package, "TO-$1").to_string();
    let remaining = format!("{}{}", &query[..m.start()], &query[m.end()..]).trim().to_string();

    if kind == "mxx_diode_pkg" {
        if package == "M4" || package == "M7" {
            package = "SMA".to_string();
        } else if package == "M8" {
            package = "SMB".to_string();
        }
    }

    if kind == "usb" {
        // USB-C/TYPE-C are not JLCPCB package names (their package is "SMD") — they're
        // connector types, so keep them in the query for FTS instead of using them as
        // a package filter.
        return (None, query.to_string(), Some("usb connectors".to_string()));
    }

    (Some(package), remaining, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_extraction() {
        for (query, expected_package, expected_remaining) in [
            ("30V N-Channel MOSFET SO-8", "SO-8", "30V N-Channel MOSFET"),
            ("mosfet SO8", "SO8", "mosfet"),
            ("SOP-8 mosfet", "SOP-8", "mosfet"),
            ("SOIC-8 driver", "SOIC-8", "driver"),
            ("10k resistor 0603", "0603", "10k resistor"),
            ("SOT-23 mosfet", "SOT-23", "mosfet"),
            ("QFN-24 mcu", "QFN-24", "mcu"),
            ("DIP-8 opamp", "DIP-8", "opamp"),
            ("NPN SOT23", "SOT-23", "NPN"),
            ("SOD323 diode", "SOD-323", "diode"),
            ("QFN32 mcu", "QFN32", "mcu"),
        ] {
            let (pkg, remaining, _suggested) = extract_package(query);
            let pkg = pkg.unwrap_or_else(|| panic!("should extract package from '{query}'"));
            assert_eq!(pkg.to_uppercase(), expected_package.to_uppercase());
            assert_eq!(remaining.trim(), expected_remaining.trim());
        }
    }

    #[test]
    fn usb_c_suggests_subcategory_without_becoming_package() {
        let (pkg, remaining, suggested) = extract_package("USB-C connector");
        assert_eq!(pkg, None);
        assert_eq!(remaining, "USB-C connector");
        assert_eq!(suggested, Some("usb connectors".to_string()));
    }

    #[test]
    fn sma_diode_package_matched_when_not_followed_by_connector() {
        // Regression case for the negative-lookahead workaround: bare "SMA" (a diode
        // package) is matched; "SMA connector" is not treated as the diode package.
        let (pkg, _remaining, _suggested) = extract_package("SMA diode 1A");
        assert_eq!(pkg, Some("SMA".to_string()));
    }

    #[test]
    fn sma_connector_not_matched_as_diode_package() {
        let (pkg, _remaining, _suggested) = extract_package("SMA connector coax");
        assert_ne!(pkg, Some("SMA".to_string()));
    }
}
