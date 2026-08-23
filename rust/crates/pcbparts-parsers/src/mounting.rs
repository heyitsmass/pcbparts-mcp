use std::collections::HashSet;

fn non_pcb_categories() -> HashSet<&'static str> {
    HashSet::from([
        "Building materials / Building hardware",
        "Consumables and auxiliary materials",
        "Development Boards & Tools",
        "Hardware Fasteners",
        "Lathes and accessories",
        "Office Daily Use",
        "Pneumatic/hydraulic/valves/pumps",
        "Tool Equipment",
        "Wires and cables",
    ])
}

fn category_smd_patterns() -> &'static [&'static str] {
    &["SMD", "SMT", "SURFACE MOUNT"]
}

fn category_through_hole_patterns() -> &'static [&'static str] {
    &["THROUGH HOLE", "THROUGH-HOLE"]
}

fn smd_patterns() -> &'static [&'static str] {
    &[
        "0201", "0402", "0603", "0805", "1206", "1210", "1812", "2010", "2512",
        "01005", "008004",
        "SOT", "SOD", "SOP", "SOIC", "SSOP", "TSSOP", "TSOP", "MSOP",
        "SO-",
        "QFP", "TQFP", "LQFP", "PQFP", "VQFP", "SQFP",
        "QFN", "DFN", "MLF", "SON", "WSON", "UDFN", "VDFN",
        "BGA", "CSP", "WLCSP", "FCBGA", "FBGA", "PBGA", "UBGA",
        "LGA", "PLCC",
        "TO-252", "TO-263", "TO-277", "DPAK", "D2PAK", "D3PAK",
        "DO-214", "DO-218", "SMA", "SMB", "SMC",
        "SC-70", "SC-88", "SC-89",
        "LL-34", "LL-41", "MINIMELF", "MELF",
        "MC-306", "MC-146", "MC-156", "DT-26", "DT-38",
        "CASE-",
        "EIA-",
    ]
}

fn through_hole_patterns() -> &'static [&'static str] {
    &[
        "DIP", "PDIP", "CDIP", "CERDIP",
        "SIP",
        "TO-92", "TO-126", "TO-220", "TO-247", "TO-264", "TO-3",
        "DO-41", "DO-35", "DO-201", "DO-15", "DO-27",
        "R-1", "R-6",
        "PIN", "THT", "AXIAL", "RADIAL",
        "PLUGIN",
        "P=",
        "HC-49", "HC-50", "HC-51", "HC-52",
        "THROUGH HOLE", "THROUGH-HOLE",
        "PUSH-PULL",
        "KBP", "KBL", "KBU", "KBPC", "MBS", "MBF", "GBU", "DBS", "GBJ", "BR-",
        "插件", "弯插", "直插",
    ]
}

pub fn detect_mounting_type(
    package: Option<&str>,
    category: Option<&str>,
    subcategory: Option<&str>,
) -> &'static str {
    if let Some(cat) = category {
        if non_pcb_categories().contains(cat) {
            return "not_applicable";
        }
    }

    if let Some(sub) = subcategory {
        let sub_upper = sub.to_uppercase();
        for pattern in category_through_hole_patterns() {
            if sub_upper.contains(pattern) {
                return "through_hole";
            }
        }
        for pattern in category_smd_patterns() {
            if sub_upper.contains(pattern) {
                return "smd";
            }
        }
    }

    if let Some(cat) = category {
        let cat_upper = cat.to_uppercase();
        for pattern in category_through_hole_patterns() {
            if cat_upper.contains(pattern) {
                return "through_hole";
            }
        }
        for pattern in category_smd_patterns() {
            if cat_upper.contains(pattern) {
                return "smd";
            }
        }
    }

    let package = match package {
        Some(p) if !p.is_empty() => p,
        _ => return "not_sure",
    };

    let pkg_upper = package.to_uppercase();

    if pkg_upper.contains("SMD") || pkg_upper.contains("SMT") {
        return "smd";
    }

    for pattern in through_hole_patterns() {
        if pkg_upper.contains(pattern) {
            return "through_hole";
        }
    }

    for pattern in smd_patterns() {
        if pkg_upper.contains(pattern) {
            return "smd";
        }
    }

    if pkg_upper.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return "smd";
    }

    "not_sure"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smd_packages() {
        for package in [
            "0402", "0603", "0805", "1206", "1210",
            "SOT-23", "SOT-23-5", "SOT-223", "SOT-89",
            "SOIC-8", "SOP-8", "SSOP-16", "TSSOP-20",
            "QFP-48", "LQFP-64", "TQFP-32",
            "QFN-24", "DFN-8", "WSON-8",
            "BGA-256", "WLCSP-20",
            "DPAK", "TO-252", "TO-263", "D2PAK",
            "DO-214AC", "SMA", "SMB", "SMC",
            "SC-70-5", "SC-88",
            "SMD,4x3mm",
            "CASE-A", "CASE-B", "CASE-C", "CASE-D",
            "EIA-3216", "EIA-3528-21",
        ] {
            assert_eq!(detect_mounting_type(Some(package), None, None), "smd", "{package}");
        }
    }

    #[test]
    fn test_through_hole_packages() {
        for package in [
            "DIP-8", "DIP-16", "PDIP-28",
            "TO-220", "TO-220-3", "TO-92", "TO-247",
            "DO-41", "DO-35", "DO-201AD",
            "SIP-3", "SIP-9",
            "Axial", "AXIAL-0.3",
            "Radial", "RADIAL-5mm",
            "PIN Header", "2.54mm,Pin Header",
            "Plugin", "Plugin,P=2.54mm", "Plugin,D=5mm",
            "HC-49S", "HC-49U",
            "Through hole", "Through-hole",
            "Push-Pull,P=2.54mm",
        ] {
            assert_eq!(detect_mounting_type(Some(package), None, None), "through_hole", "{package}");
        }
    }

    #[test]
    fn test_empty_package_defaults_to_not_sure() {
        assert_eq!(detect_mounting_type(Some(""), None, None), "not_sure");
        assert_eq!(detect_mounting_type(None, None, None), "not_sure");
    }

    #[test]
    fn test_case_insensitive() {
        assert_eq!(detect_mounting_type(Some("qfn-24"), None, None), "smd");
        assert_eq!(detect_mounting_type(Some("QFN-24"), None, None), "smd");
        assert_eq!(detect_mounting_type(Some("dip-8"), None, None), "through_hole");
        assert_eq!(detect_mounting_type(Some("DIP-8"), None, None), "through_hole");
    }

    #[test]
    fn test_unknown_defaults_to_not_sure() {
        assert_eq!(detect_mounting_type(Some("CUSTOM-PKG"), None, None), "not_sure");
        assert_eq!(detect_mounting_type(Some("XYZ-123"), None, None), "not_sure");
        assert_eq!(detect_mounting_type(Some("-"), None, None), "not_sure");
    }

    #[test]
    fn test_smd_subcategories() {
        for subcategory in [
            "Aluminum Electrolytic Capacitors - SMD",
            "Multilayer Ceramic Capacitors MLCC - SMD/SMT",
            "Inductors (SMD)",
            "Chip Resistor - Surface Mount",
            "SMD Quick Terminal",
        ] {
            assert_eq!(detect_mounting_type(Some("UNKNOWN-PKG"), None, Some(subcategory)), "smd");
        }
    }

    #[test]
    fn test_through_hole_subcategories() {
        for subcategory in [
            "Through Hole Ceramic Capacitors",
            "Through Hole Resistors",
            "Color Ring Inductors / Through Hole Inductors",
        ] {
            assert_eq!(detect_mounting_type(Some("0402"), None, Some(subcategory)), "through_hole");
        }
    }

    #[test]
    fn test_dip_switches_uses_package_not_category() {
        assert_eq!(detect_mounting_type(Some("SMD,P=1.27mm"), None, Some("DIP Switches")), "smd");
        assert_eq!(detect_mounting_type(Some("DIP-8"), None, Some("DIP Switches")), "through_hole");
    }

    #[test]
    fn test_plugin_in_category_uses_package() {
        assert_eq!(detect_mounting_type(Some("Plugin,D5mm"), None, Some("Ceramic plugin capacitor")), "through_hole");
        assert_eq!(detect_mounting_type(Some("0402"), None, Some("Ceramic plugin capacitor")), "smd");
    }

    #[test]
    fn test_subcategory_overrides_package() {
        assert_eq!(detect_mounting_type(Some("0402"), None, Some("Through Hole Resistors")), "through_hole");
        assert_eq!(detect_mounting_type(Some("DIP-8"), None, Some("Inductors (SMD)")), "smd");
    }

    #[test]
    fn test_falls_back_to_package_when_no_category_hint() {
        assert_eq!(detect_mounting_type(Some("0402"), None, Some("Resistors")), "smd");
        assert_eq!(detect_mounting_type(Some("DIP-8"), None, Some("Resistors")), "through_hole");
    }

    #[test]
    fn test_feed_through_not_matched() {
        assert_eq!(detect_mounting_type(Some("0402"), None, Some("Feed Through Capacitors")), "smd");
    }

    #[test]
    fn test_hot_dip_not_matched() {
        assert_eq!(detect_mounting_type(Some("M3"), None, Some("Hot-dip galvanized screw")), "not_sure");
    }

    #[test]
    fn test_non_pcb_categories_return_not_applicable() {
        for category in [
            "Building materials / Building hardware",
            "Consumables and auxiliary materials",
            "Development Boards & Tools",
            "Hardware Fasteners",
            "Lathes and accessories",
            "Office Daily Use",
            "Pneumatic/hydraulic/valves/pumps",
            "Tool Equipment",
            "Wires and cables",
        ] {
            assert_eq!(detect_mounting_type(Some("0402"), Some(category), None), "not_applicable");
            assert_eq!(detect_mounting_type(Some("DIP-8"), Some(category), None), "not_applicable");
            assert_eq!(detect_mounting_type(Some("-"), Some(category), None), "not_applicable");
        }
    }

    #[test]
    fn test_pcb_category_still_detects_mounting() {
        assert_eq!(detect_mounting_type(Some("0402"), Some("Capacitors"), None), "smd");
        assert_eq!(detect_mounting_type(Some("DIP-8"), Some("Resistors"), None), "through_hole");
    }
}
