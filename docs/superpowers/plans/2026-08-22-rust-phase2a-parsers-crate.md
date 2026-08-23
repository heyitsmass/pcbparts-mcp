# Rust Migration Phase 2A: pcbparts-parsers Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the genuinely independent, foundational half of Phase 2
(`parsers.py`, `mounting.py`, `manufacturer_aliases.py`,
`subcategory_aliases.py`, `pinout.py`, `design_rules.py`) into a new
`pcbparts-parsers` Rust crate, with every existing pytest test that targets
these modules translated 1:1 into a passing Rust test.

**Architecture:** A new crate, `pcbparts-parsers`, added to the existing
`rust/` Cargo workspace (created in Phase 1). One Rust module per Python
file — `parsers.rs`, `mounting.rs`, `pinout.rs`, `design_rules.rs`,
`manufacturer_aliases.rs`, `subcategory_aliases.rs` — matching the "one file,
one responsibility" boundary the Python source already uses. Every function
here has already been written, compiled, and run (205/205 tests passing in
a scratchpad prototype, including the boards/sensor tests carried over from
Phase 1) — this plan transcribes verified code.

**Tech Stack:** Rust 2021 edition, `regex` (for the ~25 pre-compiled parsing
patterns `parsers.py` and `pinout.py` use), `serde_json` (dynamic
dict-shaped values, matching `pinout.py`'s/`design_rules.py`'s Python `dict`
returns), `tempfile` (dev-dependency, for `design_rules.rs`'s filesystem
tests).

**Spec:** `docs/superpowers/specs/2026-08-22-rust-migration-design.md`

## Global Constraints

- Every ported test must assert the same behavior as its Python counterpart
  (golden-value parity), not a re-derived expectation.
- `alternatives.py` (Phase 2B, a separate plan) depends on this crate's
  `parsers.rs` — nothing in Phase 2A depends on `alternatives.py`, so this
  plan has no forward dependency.
- `manufacturer_aliases.py` and `subcategory_aliases.py` have **no dedicated
  Python test file today** (confirmed: no `test_manufacturer_aliases.py` or
  `test_subcategory_aliases.py` exists, and neither name appears in any
  other test file). They are ported as data + the functions
  `subcategory_aliases.py` defines (`resolve_subcategory_name`,
  `find_similar_subcategories`), with **no new tests invented** — this
  matches current coverage exactly rather than silently expanding scope.
  Both are exercised only indirectly, later, through Phase 3's search
  engine.
- `test_parsers.py` and `test_pinout.py` are each split across two phases:
  the portions below are the parsers.py-only / pinout.py-only halves; the
  `smart_parser`-testing portion of `test_parsers.py` and the
  `@pytest.mark.integration class TestPinoutIntegration` portion of
  `test_pinout.py` are **not** part of this plan (Phase 4 and Phase 7
  respectively — see spec).
- Per CLAUDE.md and the `project-rust-rewrite` memory: never commit without
  explicit permission, no Claude attribution in commit messages.

## File Structure

```
rust/crates/pcbparts-parsers/
  Cargo.toml
  src/
    lib.rs                    # pub mod declarations for all 6 modules
    parsers.rs                # ~25 parse_* functions + impedance_at_freq_match
    mounting.rs                # detect_mounting_type
    pinout.rs                  # parse_easyeda_pins
    design_rules.rs            # get_design_rules
    manufacturer_aliases.rs     # KNOWN_MANUFACTURERS, MANUFACTURER_ALIASES (data only)
    subcategory_aliases.rs      # SUBCATEGORY_ALIASES + resolve_subcategory_name + find_similar_subcategories
```

---

### Task 1: Crate scaffold + parsers.rs

**Files:**
- Create: `rust/crates/pcbparts-parsers/Cargo.toml`
- Create: `rust/crates/pcbparts-parsers/src/lib.rs`
- Create: `rust/crates/pcbparts-parsers/src/parsers.rs`
- Modify: `rust/Cargo.toml` (add the new crate to `members`)

**Interfaces:**
- Produces: all `parse_*` functions plus `impedance_at_freq_match` — the
  foundation every other Task in this plan, and Phase 2B's `alternatives.rs`,
  builds on.

- [ ] **Step 1: Add the crate to the workspace**

```toml
# rust/Cargo.toml
[workspace]
resolver = "2"
members = ["crates/pcbparts-db", "crates/pcbparts-parsers"]
```

- [ ] **Step 2: Create the crate manifest**

```toml
# rust/crates/pcbparts-parsers/Cargo.toml
[package]
name = "pcbparts-parsers"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
regex = "1"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Write `lib.rs`**

```rust
pub mod parsers;
pub mod mounting;
pub mod pinout;
pub mod design_rules;
pub mod manufacturer_aliases;
pub mod subcategory_aliases;
```

- [ ] **Step 4: Write the failing tests for `parsers.rs`**

This is the full test suite ported from two Python files that both test the
same underlying `parsers.py` functions: `tests/test_parsers.py` (the
parsers.py-only portion — its `TestModelNumberExtraction` onward tests
`smart_parser`, not in scope here) and `tests/test_alternatives.py` (its
parser-level tests only — `alternatives.py` re-exports these same functions
via `from pcbparts_mcp.parsers import (...)`, so both Python files are
exercising one function each; consolidated here into one Rust test module
rather than duplicated across two).

```rust
// rust/crates/pcbparts-parsers/src/parsers.rs — tests module (write this first)
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
}
```

- [ ] **Step 5: Run the tests to verify they fail**

Run: `cd rust && cargo test -p pcbparts-parsers`
Expected: FAIL to compile — none of the `parse_*` functions exist yet.

- [ ] **Step 6: Write the implementation**

```rust
// rust/crates/pcbparts-parsers/src/parsers.rs — place above the tests module
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
    INTEGER_PATTERN.captures(s).map(|c| c[1].parse().unwrap())
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
```

Note: `parse_temperature`, `parse_temp_range`, `parse_percentage`,
`parse_wavelength`, `parse_luminosity`, `parse_capacitance_pf`,
`parse_integer`, `parse_vgs_range`, `parse_freq_range`, `parse_vin_range`
have **no direct pytest coverage in Python either** — they're ported here
(needed by `alternatives.py`/Phase 2B and later phases) without inventing
new tests, matching current coverage exactly.

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cd rust && cargo test -p pcbparts-parsers`
Expected: PASS — 32 tests (this has been verified already).

- [ ] **Step 8: Commit**

```bash
git add rust/Cargo.toml rust/crates/pcbparts-parsers
git commit -m "rust: scaffold pcbparts-parsers crate, port parsers.py"
```

---

### Task 2: mounting.rs

**Files:**
- Create: `rust/crates/pcbparts-parsers/src/mounting.rs`

**Interfaces:**
- Produces: `detect_mounting_type(package: Option<&str>, category: Option<&str>, subcategory: Option<&str>) -> &'static str`
  — consumed later by Phase 3's `search/result.rs` port (`row_to_dict`
  calls `detect_mounting_type` to populate each component's `mounting_type`
  field).

- [ ] **Step 1: Write the failing tests**

```rust
// rust/crates/pcbparts-parsers/src/mounting.rs — tests module
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p pcbparts-parsers mounting::`
Expected: FAIL to compile — `detect_mounting_type` doesn't exist yet.

- [ ] **Step 3: Write the implementation**

```rust
// rust/crates/pcbparts-parsers/src/mounting.rs — place above the tests module
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
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p pcbparts-parsers mounting::`
Expected: PASS — 15 tests (verified).

- [ ] **Step 5: Commit**

```bash
git add rust/crates/pcbparts-parsers/src/mounting.rs
git commit -m "rust: port mounting.py detect_mounting_type"
```

---

### Task 3: pinout.rs

**Files:**
- Create: `rust/crates/pcbparts-parsers/src/pinout.rs`

**Interfaces:**
- Consumes: `serde_json::Value` for the dynamic EasyEDA response shape
  (matches Python's untyped `dict`).
- Produces: `parse_easyeda_pins(data: &Value) -> Vec<Pin>`, `Pin { number:
  Option<String>, name: Option<String>, electrical: Option<String> }` —
  consumed by Phase 9 (server)'s `jlc_get_pinout` tool handler.

- [ ] **Step 1: Write the failing tests**

```rust
// rust/crates/pcbparts-parsers/src/pinout.rs — tests module
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_simple_mosfet_pins() {
        let data = json!({
            "dataStr": {
                "shape": [
                    "P~show~0~1~100~100~180~gge1~0^^100~100^^M100,100h10~#880000^^1~110~105~0~G~start~~~#880000^^1~100~100~0~1~end~~~#0000FF",
                    "P~show~0~2~100~120~180~gge2~0^^100~120^^M100,120h10~#880000^^1~110~125~0~S~start~~~#880000^^1~100~120~0~2~end~~~#0000FF",
                    "P~show~0~3~100~140~180~gge3~0^^100~140^^M100,140h10~#880000^^1~110~145~0~D~start~~~#880000^^1~100~140~0~3~end~~~#0000FF",
                ]
            }
        });
        let pins = parse_easyeda_pins(&data);
        assert_eq!(pins.len(), 3);
        assert_eq!(pins[0].number.as_deref(), Some("1"));
        assert_eq!(pins[0].name.as_deref(), Some("G"));
        assert_eq!(pins[0].electrical, None);
        assert_eq!(pins[1].name.as_deref(), Some("S"));
        assert_eq!(pins[2].name.as_deref(), Some("D"));
    }

    #[test]
    fn test_parse_numbered_only_pins() {
        let data = json!({
            "dataStr": {
                "shape": [
                    "P~show~0~1~100~100~180~gge1~0^^100~100^^M100,100h10~#0000FF^^1~110~105~0~1~start~~~#0000FF^^1~100~100~0~1~end~~~#0000FF",
                    "P~show~0~2~100~120~180~gge2~0^^100~120^^M100,120h10~#0000FF^^1~110~125~0~2~start~~~#0000FF^^1~100~120~0~2~end~~~#0000FF",
                ]
            }
        });
        let pins = parse_easyeda_pins(&data);
        assert_eq!(pins.len(), 2);
        assert_eq!(pins[0].number.as_deref(), Some("1"));
        assert_eq!(pins[0].name.as_deref(), Some("1"));
        assert_eq!(pins[0].electrical, None);
    }

    #[test]
    fn test_parse_with_electrical_types() {
        let data = json!({
            "dataStr": {
                "shape": [
                    "P~show~3~1~100~100~180~gge1~0^^100~100^^M100,100h10~#880000^^1~110~105~0~1~start~~~#880000^^1~100~100~0~1~end~~~#0000FF",
                    "P~show~1~2~100~120~180~gge2~0^^100~120^^M100,120h10~#880000^^1~110~125~0~2~start~~~#880000^^1~100~120~0~2~end~~~#0000FF",
                    "P~show~2~3~100~140~180~gge3~0^^100~140^^M100,140h10~#880000^^1~110~145~0~3~start~~~#880000^^1~100~140~0~3~end~~~#0000FF",
                    "P~show~4~4~100~160~180~gge4~0^^100~160^^M100,160h10~#880000^^1~110~165~0~4~start~~~#880000^^1~100~160~0~4~end~~~#0000FF",
                ]
            }
        });
        let pins = parse_easyeda_pins(&data);
        assert_eq!(pins.len(), 4);
        assert_eq!(pins[0].electrical.as_deref(), Some("bidirectional"));
        assert_eq!(pins[1].electrical.as_deref(), Some("input"));
        assert_eq!(pins[2].electrical.as_deref(), Some("output"));
        assert_eq!(pins[3].electrical.as_deref(), Some("power"));
    }

    #[test]
    fn test_parse_named_pins() {
        let data = json!({
            "dataStr": {
                "shape": [
                    "P~show~0~1~100~100~180~gge1~0^^100~100^^M100,100h10~#FF0000^^1~110~105~0~VDD~start~~~#FF0000^^1~100~100~0~1~end~~~#0000FF",
                    "P~show~0~2~100~120~180~gge2~0^^100~120^^M100,120h10~#000000^^1~110~125~0~GND~start~~~#000000^^1~100~120~0~2~end~~~#0000FF",
                    "P~show~0~3~100~140~180~gge3~0^^100~140^^M100,140h10~#880000^^1~110~145~0~PA0~start~~~#880000^^1~100~140~0~3~end~~~#0000FF",
                ]
            }
        });
        let pins = parse_easyeda_pins(&data);
        assert_eq!(pins.len(), 3);
        assert_eq!(pins[0].name.as_deref(), Some("VDD"));
        assert_eq!(pins[1].name.as_deref(), Some("GND"));
        assert_eq!(pins[2].name.as_deref(), Some("PA0"));
    }

    #[test]
    fn test_parse_complex_stm32_name() {
        let data = json!({
            "dataStr": {
                "shape": [
                    "P~show~0~1~100~100~180~gge1~0^^100~100^^M100,100h10~#880000^^1~110~105~0~PC13-TAMPER-RTC~start~~~#880000^^1~100~100~0~1~end~~~#0000FF",
                ]
            }
        });
        let pins = parse_easyeda_pins(&data);
        assert_eq!(pins.len(), 1);
        assert_eq!(pins[0].name.as_deref(), Some("PC13-TAMPER-RTC"));
    }

    #[test]
    fn test_parse_empty_shape() {
        let data = json!({"dataStr": {"shape": []}});
        assert_eq!(parse_easyeda_pins(&data), vec![]);
    }

    #[test]
    fn test_parse_missing_datastr() {
        let data = json!({});
        assert_eq!(parse_easyeda_pins(&data), vec![]);
    }

    #[test]
    fn test_parse_string_datastr() {
        let inner = json!({
            "shape": [
                "P~show~0~1~100~100~180~gge1~0^^100~100^^M100,100h10~#880000^^1~110~105~0~VCC~start~~~#880000^^1~100~100~0~1~end~~~#0000FF",
            ]
        });
        let data = json!({"dataStr": inner.to_string()});
        let pins = parse_easyeda_pins(&data);
        assert_eq!(pins.len(), 1);
        assert_eq!(pins[0].name.as_deref(), Some("VCC"));
    }

    #[test]
    fn test_parse_pins_sorted_by_number() {
        let data = json!({
            "dataStr": {
                "shape": [
                    "P~show~0~3~100~100~180~gge1~0^^100~100^^M100,100h10~#880000^^1~110~105~0~C~start~~~#880000^^1~100~100~0~3~end~~~#0000FF",
                    "P~show~0~1~100~120~180~gge2~0^^100~120^^M100,120h10~#880000^^1~110~125~0~A~start~~~#880000^^1~100~120~0~1~end~~~#0000FF",
                    "P~show~0~2~100~140~180~gge3~0^^100~140^^M100,140h10~#880000^^1~110~145~0~B~start~~~#880000^^1~100~140~0~2~end~~~#0000FF",
                ]
            }
        });
        let pins = parse_easyeda_pins(&data);
        let numbers: Vec<&str> = pins.iter().map(|p| p.number.as_deref().unwrap()).collect();
        assert_eq!(numbers, vec!["1", "2", "3"]);
        let names: Vec<&str> = pins.iter().map(|p| p.name.as_deref().unwrap()).collect();
        assert_eq!(names, vec!["A", "B", "C"]);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p pcbparts-parsers pinout::`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

```rust
// rust/crates/pcbparts-parsers/src/pinout.rs — place above the tests module
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

static START_LABEL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"~([^~]+)~start~~~").unwrap());
static END_LABEL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"~([^~]+)~end~~~").unwrap());

fn electrical_type(code: &str) -> &'static str {
    match code {
        "1" => "input",
        "2" => "output",
        "3" => "bidirectional",
        "4" => "power",
        _ => "undefined",
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Pin {
    pub number: Option<String>,
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub electrical: Option<String>,
}

/// Sort key mirroring Python's `_sort_key`: numeric pin numbers sort first (by
/// value), non-numeric ones sort after (lexicographically). Encoded as a tuple
/// so numeric entries (group 0) always precede text entries (group 1) and the
/// unused third/second field stays a fixed default within each group.
fn sort_key(num: &Option<String>) -> (u8, i64, String) {
    match num.as_deref().and_then(|s| s.parse::<i64>().ok()) {
        Some(n) => (0, n, String::new()),
        None => (1, 0, num.clone().unwrap_or_default()),
    }
}

/// Parse pin data from an EasyEDA component response. Returns raw pin data
/// exactly as EasyEDA provides it, with no interpretation.
pub fn parse_easyeda_pins(data: &Value) -> Vec<Pin> {
    let data_str_raw = data.get("dataStr").cloned().unwrap_or(Value::Null);

    let data_str: Value = match &data_str_raw {
        Value::String(s) => match serde_json::from_str(s) {
            Ok(v) => v,
            Err(_) => return vec![],
        },
        other => other.clone(),
    };

    let Some(obj) = data_str.as_object() else {
        return vec![];
    };
    let Some(shape) = obj.get("shape").and_then(|v| v.as_array()) else {
        return vec![];
    };
    if shape.is_empty() {
        return vec![];
    }

    let mut pins: Vec<Pin> = Vec::new();

    for element in shape {
        let Some(element) = element.as_str() else {
            continue;
        };
        if !element.starts_with("P~") {
            continue;
        }

        let parts: Vec<&str> = element.split('~').collect();
        let electric_code = parts.get(2).copied().unwrap_or("");
        let pin_num = parts.get(3).map(|s| s.to_string());

        let start_label = START_LABEL_PATTERN
            .captures(element)
            .map(|c| c[1].to_string());
        let end_label = END_LABEL_PATTERN
            .captures(element)
            .map(|c| c[1].to_string());

        let is_digit = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());

        let pin_name = if start_label.as_deref().is_some_and(|s| !is_digit(s)) {
            start_label
        } else if end_label.as_deref().is_some_and(|s| !is_digit(s)) {
            end_label
        } else {
            pin_num.clone()
        };

        let electrical = electrical_type(electric_code);
        pins.push(Pin {
            number: pin_num,
            name: pin_name,
            electrical: if electrical != "undefined" {
                Some(electrical.to_string())
            } else {
                None
            },
        });
    }

    pins.sort_by_key(|p| sort_key(&p.number));
    pins
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p pcbparts-parsers pinout::`
Expected: PASS — 9 tests (verified).

- [ ] **Step 5: Commit**

```bash
git add rust/crates/pcbparts-parsers/src/pinout.rs
git commit -m "rust: port pinout.py parse_easyeda_pins"
```

---

### Task 4: design_rules.rs

**Files:**
- Create: `rust/crates/pcbparts-parsers/src/design_rules.rs`

**Interfaces:**
- Produces: `get_design_rules(topic: &str, rules_dir: Option<&Path>) ->
  serde_json::Value` — consumed by Phase 9 (server)'s `get_design_rules`
  tool handler (calling with `rules_dir: None`, relying on the
  `DESIGN_RULES_DIR` env var / default path + module-level cache, exactly
  like Python's production call path).

- [ ] **Step 1: Write the failing tests**

Every existing Python test passes an explicit `rules_dir` — none exercises
the default-directory/cache code path (same as Python: it's implemented for
production use, not covered by these tests).

```rust
// rust/crates/pcbparts-parsers/src/design_rules.rs — tests module
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn rules_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("INDEX.md"), "# Design Rules Index\n\nAll the rules.").unwrap();

        let power = root.join("power");
        fs::create_dir(&power).unwrap();
        fs::write(power.join("ldo.md"), "# LDO Design Rules\nDropout, ESR stability.").unwrap();
        fs::write(power.join("switching.md"), "# Switching Regulator Rules\nBuck/boost topology.").unwrap();

        let ifaces = root.join("interfaces");
        fs::create_dir(&ifaces).unwrap();
        fs::write(ifaces.join("usb.md"), "# USB Design Rules\nUSB-C CC resistors.").unwrap();
        fs::write(ifaces.join("i2c.md"), "# I2C Design Rules\nPull-up calculation.").unwrap();

        let mcus = root.join("mcus");
        fs::create_dir(&mcus).unwrap();
        fs::write(mcus.join("esp32.md"), "# ESP32 Design Rules\nStrapping pins.").unwrap();

        dir
    }

    #[test]
    fn test_empty_topic_returns_index() {
        let dir = rules_dir();
        let result = get_design_rules("", Some(dir.path()));
        assert!(result["content"].as_str().unwrap().contains("Design Rules Index"));
        assert_eq!(result["matched_files"], json!(["INDEX.md"]));
        assert_eq!(result["topic"], "");
    }

    #[test]
    fn test_single_match() {
        let dir = rules_dir();
        let result = get_design_rules("ldo", Some(dir.path()));
        assert!(result["content"].as_str().unwrap().contains("LDO Design Rules"));
        assert_eq!(result["matched_files"], json!(["power/ldo"]));
        assert_eq!(result["topic"], "ldo");
    }

    #[test]
    fn test_category_match() {
        let dir = rules_dir();
        let result = get_design_rules("power", Some(dir.path()));
        let content = result["content"].as_str().unwrap();
        assert!(content.contains("LDO Design Rules"));
        assert!(content.contains("Switching Regulator Rules"));
        assert_eq!(result["matched_files"].as_array().unwrap().len(), 2);
        let files: Vec<&str> = result["matched_files"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert!(files.contains(&"power/ldo"));
        assert!(files.contains(&"power/switching"));
    }

    #[test]
    fn test_no_match_returns_index() {
        let dir = rules_dir();
        let result = get_design_rules("nonexistent", Some(dir.path()));
        assert!(result["content"].as_str().unwrap().contains("No rules found matching 'nonexistent'"));
        assert!(result["content"].as_str().unwrap().contains("Design Rules Index"));
        assert_eq!(result["matched_files"], json!([]));
    }

    #[test]
    fn test_case_insensitive() {
        let dir = rules_dir();
        let result = get_design_rules("LDO", Some(dir.path()));
        assert!(result["content"].as_str().unwrap().contains("LDO Design Rules"));
        assert_eq!(result["matched_files"], json!(["power/ldo"]));
    }

    #[test]
    fn test_partial_match() {
        let dir = rules_dir();
        let result = get_design_rules("usb", Some(dir.path()));
        assert!(result["content"].as_str().unwrap().contains("USB Design Rules"));
        assert_eq!(result["matched_files"], json!(["interfaces/usb"]));
    }

    #[test]
    fn test_separator_format() {
        let dir = rules_dir();
        let result = get_design_rules("power", Some(dir.path()));
        assert!(result["content"].as_str().unwrap().contains("\n\n---\n\n"));
    }

    #[test]
    fn test_matched_files_field() {
        let dir = rules_dir();
        let result = get_design_rules("i2c", Some(dir.path()));
        assert_eq!(result["matched_files"], json!(["interfaces/i2c"]));
    }

    #[test]
    fn test_missing_dir() {
        let result = get_design_rules("ldo", Some(Path::new("/nonexistent/path")));
        assert!(result.get("error").is_some());
        assert!(result["error"].as_str().unwrap().contains("not available"));
        assert_eq!(result["matched_files"], json!([]));
    }

    #[test]
    fn test_whitespace_topic_returns_index() {
        let dir = rules_dir();
        let result = get_design_rules("  ", Some(dir.path()));
        assert!(result["content"].as_str().unwrap().contains("Design Rules Index"));
        assert_eq!(result["matched_files"], json!(["INDEX.md"]));
    }

    #[test]
    fn test_broad_match_returns_file_list() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("INDEX.md"), "# Index").unwrap();
        for name in ["a-power.md", "b-power.md", "c-power.md", "d-power.md"] {
            fs::write(dir.path().join(name), format!("# {name}\nContent of {name}.")).unwrap();
        }
        let result = get_design_rules("power", Some(dir.path()));
        assert_eq!(result["matched_files"].as_array().unwrap().len(), 4);
        assert!(result["content"].as_str().unwrap().contains("Found 4 rule files"));
        assert!(result["content"].as_str().unwrap().contains("more specific topic"));
        assert!(!result["content"].as_str().unwrap().contains("Content of"));
    }

    #[test]
    fn test_three_matches_returns_full_content() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("INDEX.md"), "# Index").unwrap();
        for name in ["a-power.md", "b-power.md", "c-power.md"] {
            fs::write(dir.path().join(name), format!("# {name}\nContent of {name}.")).unwrap();
        }
        let result = get_design_rules("power", Some(dir.path()));
        assert_eq!(result["matched_files"].as_array().unwrap().len(), 3);
        assert!(result["content"].as_str().unwrap().contains("Content of"));
    }

    #[test]
    fn test_alias_resolution() {
        let dir = rules_dir();
        let result = get_design_rules("buck", Some(dir.path()));
        assert_eq!(result["matched_files"], json!(["power/switching"]));
        assert!(result["content"].as_str().unwrap().contains("Switching Regulator Rules"));
    }

    #[test]
    fn test_hyphenated_topic() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("INDEX.md"), "# Index").unwrap();
        let misc = dir.path().join("misc");
        fs::create_dir(&misc).unwrap();
        fs::write(misc.join("op-amp-basics.md"), "# Op-Amp Basics\nSelection guide.").unwrap();
        let result = get_design_rules("op-amp", Some(dir.path()));
        assert_eq!(result["matched_files"], json!(["misc/op-amp-basics"]));
        assert!(result["content"].as_str().unwrap().contains("Op-Amp Basics"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p pcbparts-parsers design_rules::`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

```rust
// rust/crates/pcbparts-parsers/src/design_rules.rs — place above the tests module
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

fn excluded_files() -> HashSet<&'static str> {
    HashSet::from(["INDEX.MD", "DESIGN-STYLE-GUIDE.MD", "VERIFIED-SOURCES.MD"])
}

fn aliases() -> HashMap<&'static str, Vec<&'static str>> {
    HashMap::from([
        ("buck", vec!["power/switching"]),
        ("boost", vec!["power/switching"]),
        ("flyback", vec!["power/switching"]),
        ("buck-boost", vec!["power/switching"]),
        ("usb-c", vec!["interfaces/usb"]),
        ("usbc", vec!["interfaces/usb"]),
        ("opamp", vec!["misc/op-amp-basics"]),
        ("fet", vec!["misc/mosfet-circuits"]),
        ("nmos", vec!["misc/mosfet-circuits"]),
        ("pmos", vec!["misc/mosfet-circuits"]),
        ("jtag", vec!["guides/test-debug"]),
        ("swd", vec!["guides/test-debug"]),
        ("gps", vec!["misc/gnss-integration"]),
        ("impedance", vec!["guides/signal-integrity"]),
        ("differential", vec!["guides/signal-integrity"]),
        ("crosstalk", vec!["guides/signal-integrity"]),
        ("termination", vec!["guides/signal-integrity"]),
        ("ground", vec!["guides/pcb-layout"]),
        ("stackup", vec!["guides/pcb-layout"]),
        ("trace", vec!["guides/pcb-layout"]),
        ("via", vec!["guides/pcb-layout"]),
        ("pmic", vec!["guides/power-architecture"]),
        ("sequencing", vec!["guides/power-architecture"]),
        ("inrush", vec!["guides/power-architecture"]),
        ("aliasing", vec!["misc/adc-dac"]),
        ("sampling", vec!["misc/adc-dac"]),
    ])
}

const MAX_FULL_CONTENT: usize = 3;

fn default_rules_dir() -> PathBuf {
    std::env::var("DESIGN_RULES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data/design-rules/rules"))
}

static INDEX_CACHE: OnceLock<Mutex<Option<HashMap<String, PathBuf>>>> = OnceLock::new();

fn walk_md_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            walk_md_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
}

fn build_index(rules_dir: &Path) -> HashMap<String, PathBuf> {
    let mut files = Vec::new();
    walk_md_files(rules_dir, &mut files);
    files.sort();

    let excluded = excluded_files();
    let mut idx = HashMap::new();
    for p in files {
        let name_upper = p
            .file_name()
            .map(|n| n.to_string_lossy().to_uppercase())
            .unwrap_or_default();
        if excluded.contains(name_upper.as_str()) {
            continue;
        }
        let rel = p.strip_prefix(rules_dir).unwrap_or(&p);
        let rel_no_ext = rel.with_extension("");
        idx.insert(rel_no_ext.to_string_lossy().replace('\\', "/"), p.clone());
    }
    idx
}

/// Split a key like "interfaces/rf-antenna" into ["interfaces", "rf", "antenna"]
/// and check whether `word` matches one of those tokens exactly (word-boundary
/// match, avoiding substring false positives like "rf" matching "interfaces").
fn match_word(word: &str, key: &str) -> bool {
    key.split('/')
        .flat_map(|segment| segment.split('-'))
        .any(|token| token == word)
}

pub fn get_design_rules(topic: &str, rules_dir: Option<&Path>) -> Value {
    let owned_default;
    let rd: &Path = match rules_dir {
        Some(p) => p,
        None => {
            owned_default = default_rules_dir();
            &owned_default
        }
    };

    if !rd.is_dir() {
        return json!({
            "error": "Design rules are not available.",
            "matched_files": [],
            "topic": topic,
        });
    }

    let idx: HashMap<String, PathBuf> = if rules_dir.is_some() {
        build_index(rd)
    } else {
        let cache = INDEX_CACHE.get_or_init(|| Mutex::new(None));
        let mut guard = cache.lock().unwrap();
        if guard.is_none() {
            *guard = Some(build_index(rd));
        }
        guard.clone().unwrap()
    };

    let index_path = rd.join("INDEX.md");

    if topic.trim().is_empty() {
        let content = std::fs::read_to_string(&index_path)
            .unwrap_or_else(|_| "No INDEX.md found.".to_string());
        return json!({"content": content, "matched_files": ["INDEX.md"], "topic": ""});
    }

    let topic: String = topic.chars().take(500).collect();

    let words: Vec<String> = topic
        .trim()
        .to_lowercase()
        .split(|c: char| c.is_whitespace() || c == '-')
        .filter(|w| !w.is_empty())
        .map(|s| s.to_string())
        .collect();

    let alias_map = aliases();
    let mut alias_keys: HashSet<&str> = HashSet::new();
    let mut unresolved_words: Vec<String> = Vec::new();
    for w in &words {
        if let Some(targets) = alias_map.get(w.as_str()) {
            alias_keys.extend(targets.iter().copied());
        } else {
            unresolved_words.push(w.clone());
        }
    }

    let mut matched_set: HashMap<String, PathBuf> = HashMap::new();
    for (k, p) in &idx {
        if alias_keys.contains(k.as_str()) {
            matched_set.insert(k.clone(), p.clone());
        } else if !unresolved_words.is_empty()
            && unresolved_words
                .iter()
                .any(|w| match_word(w, &k.to_lowercase()))
        {
            matched_set.insert(k.clone(), p.clone());
        }
    }

    let mut matches: Vec<(String, PathBuf)> = matched_set.into_iter().collect();
    matches.sort_by(|a, b| a.0.cmp(&b.0));

    if matches.is_empty() {
        let index_content = std::fs::read_to_string(&index_path).unwrap_or_default();
        let content = format!("No rules found matching '{topic}'. Available rules:\n\n{index_content}");
        return json!({"content": content, "matched_files": [], "topic": topic});
    }

    let matched_keys: Vec<String> = matches.iter().map(|(k, _)| k.clone()).collect();

    if matches.len() > MAX_FULL_CONTENT {
        let list = matched_keys
            .iter()
            .map(|k| format!("- {k}"))
            .collect::<Vec<_>>()
            .join("\n");
        let content = format!(
            "Found {} rule files matching '{topic}'. Call again with a more specific topic to get full content:\n\n{list}",
            matches.len()
        );
        return json!({"content": content, "matched_files": matched_keys, "topic": topic});
    }

    let mut parts = Vec::new();
    for (key, path) in &matches {
        match std::fs::read_to_string(path) {
            Ok(text) => parts.push(text),
            Err(_) => parts.push(format!("(File {key} could not be read)")),
        }
    }
    let content = parts.join("\n\n---\n\n");
    json!({"content": content, "matched_files": matched_keys, "topic": topic})
}
```

Note: this depends on `use serde_json::json;` and `use std::path::Path;`
already being visible in the test module — the test module's `use
super::*;` plus its own explicit imports cover this (see the plan's Step 1
code, which the real file combines with this implementation in one file).

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p pcbparts-parsers design_rules::`
Expected: PASS — 15 tests (verified).

- [ ] **Step 5: Commit**

```bash
git add rust/crates/pcbparts-parsers/src/design_rules.rs
git commit -m "rust: port design_rules.py get_design_rules"
```

---

### Task 5: manufacturer_aliases.rs

**Files:**
- Create: `rust/crates/pcbparts-parsers/src/manufacturer_aliases.rs`

**Interfaces:**
- Produces: `known_manufacturers() -> HashSet<&'static str>` (142 entries),
  `manufacturer_aliases() -> HashMap<&'static str, &'static str>` (164
  entries) — consumed by Phase 3's `search/resolvers.rs::resolve_manufacturer`.
- No Python test file exists for this module (`manufacturer_aliases.py`) —
  no Rust tests are added here either, matching current coverage. Data
  fidelity was verified by diffing every key/value pair against the Python
  source programmatically (142/142 and 164/164 entries matched exactly,
  zero diff) rather than by a pytest port, since none exists to port.

- [ ] **Step 1: Write the data as plain functions (no test-first cycle —
  there's no failing test to write; this is pure data transcription,
  verified by the diff check in Step 2 instead)**

```rust
// rust/crates/pcbparts-parsers/src/manufacturer_aliases.rs
use std::collections::{HashMap, HashSet};

pub fn known_manufacturers() -> HashSet<&'static str> {
    HashSet::from([
        // Passives (resistors, capacitors, inductors)
        "YAGEO", "Samsung Electro-Mechanics", "Murata Electronics", "TDK Corporation",
        "Panasonic", "Wurth Elektronik", "Bourns", "Littelfuse", "KEMET",
        "UNI-ROYAL(Uniroyal Elec)", "FH (Guangdong Fenghua Advanced Tech)",
        "FOJAN", "CCTC", "HRE", "HKR(Hong Kong Resistors)", "RALEC", "Sunlord",
        // Semiconductors
        "Texas Instruments", "STMicroelectronics", "NXP Semicon", "Microchip Tech",
        "Analog Devices", "Analog Devices Inc./Maxim Integrated", "onsemi",
        "Infineon Technologies", "Infineon/Cypress Semicon", "Renesas Electronics",
        "ROHM Semicon", "Vishay Intertech", "Diodes Incorporated", "Nexperia",
        "Broadcom Limited", "Torex Semicon", "TOSHIBA",
        // Chinese semiconductors
        "SGMICRO", "XLSEMI", "GOFORD", "3PEAK", "INJOINIC", "TECH PUBLIC",
        "UMW(Youtai Semiconductor Co., Ltd.)", "Wuxi NCE Power Semiconductor",
        "Wuxi Chipown Micro-electronics", "MICRONE(Nanjing Micro One Elec)",
        "Advanced Monolithic Systems", "HXY MOSFET", "Leiditech",
        // Transistors / MOSFETs
        "Alpha & Omega Semicon", "Guangdong Hottech", "TWGMC", "Shikues", "LRC",
        "hongjiacheng", "JSMSEMI", "MSKSEMI", "GOODWORK", "FOSAN", "YONGYUTAI",
        "MDD(Microdiode Semiconductor)", "Slkor(SLKORMICRO Elec.)",
        // MCUs
        "GigaDevice Semicon Beijing", "WCH(Jiangsu Qin Heng)", "ARTERY", "Geehy",
        "PUYA", "Nuvoton Tech", "PADAUK Tech", "FMD(Fremont Micro Devices)",
        "HK", "CW", "STC Micro", "Holtek Semicon", "Espressif Systems",
        "Nordic Semicon", "Silicon Labs", "Raspberry Pi",
        // LEDs / Optoelectronics
        "Everlight Elec", "OSRAM Opto Semicon", "Lite-On", "Sharp Microelectronics",
        "Worldsemi", "Foshan NationStar Optoelectronics", "XINGLIGHT", "Kingbright",
        "CREE LED", "TUOZHAN", "Yongyu Photoelectric", "Hubei KENTO Elec",
        // Connectors
        "JST", "MOLEX", "CJT(Changjiang Connectors)", "Korean Hroparts Elec",
        "HANRUN(Zhongshan HanRun Elec)", "SHOU HAN", "Amphenol ICC",
        "TE Connectivity", "XKB Connection", "Shenzhen Kinghelm Elec",
        "Ningbo Kangnex Elec", "BOOMELE(Boom Precision Elec)",
        // Crystals / Oscillators
        "YXC Crystal Oscillators", "Seiko", "TAITIEN Elec", "Hosonic Elec",
        // Memory
        "Winbond Elec", "Micron Tech", "ISSI(Integrated Silicon Solution)",
        "MXIC(Macronix)", "RAMXEED/FUJITSU",
        // Power / Motor drivers
        "Allegro MicroSystems, LLC", "Richtek Tech", "MaxLinear",
        // Relays
        "Omron Electronics", "HF(Xiamen Hongfa Electroacoustic)",
        "Ningbo Songle Relay", "Zhejiang HKE",
        // Sensors
        "Sensirion", "TDK InvenSense",
        // Battery Management
        "ShangHai Consonance Elec",
        // Circuit Protection / ESD
        "RUILON(Shenzhen Ruilongyuan Elec)", "Seaward Elec", "BORN", "Brightking",
        "DOWO", "Shandong Jingdao Microelectronics", "TECHFUSE",
        "Jinrui Electronic Materials Co.",
        // Other Chinese manufacturers
        "SXN(Shun Xiang Nuo Elec)", "Jiangsu Changjing Electronics Technology Co., Ltd.",
        "UTC(Unisonic Tech)", "BL(Shanghai Belling)", "OB(On-Bright Elec)",
        "HCTL", "Shanghai Prisemi Elec", "JRC", "ST(Semtech)", "Pulse Elec",
        "FUXINSEMI", "KUU", "LUTE", "Walter Elec", "Xucheng Elec",
    ])
}

pub fn manufacturer_aliases() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        // Major semiconductor companies
        ("ti", "Texas Instruments"),
        ("texas", "Texas Instruments"),
        ("stm", "STMicroelectronics"),
        ("st", "STMicroelectronics"),
        ("stmicro", "STMicroelectronics"),
        ("nxp", "NXP Semicon"),
        ("nxp semiconductor", "NXP Semicon"),
        ("microchip", "Microchip Tech"),
        ("microchip technology", "Microchip Tech"),
        ("adi", "Analog Devices"),
        ("analog", "Analog Devices"),
        ("maxim", "Analog Devices Inc./Maxim Integrated"),
        ("maxim integrated", "Analog Devices Inc./Maxim Integrated"),
        ("on", "onsemi"),
        ("on semi", "onsemi"),
        ("on semiconductor", "onsemi"),
        ("infineon", "Infineon Technologies"),
        ("cypress", "Infineon/Cypress Semicon"),
        ("renesas", "Renesas Electronics"),
        ("rohm", "ROHM Semicon"),
        ("rohm semiconductor", "ROHM Semicon"),
        ("vishay", "Vishay Intertech"),
        ("vishay intertechnology", "Vishay Intertech"),
        ("diodes", "Diodes Incorporated"),
        ("diodes inc", "Diodes Incorporated"),
        ("broadcom", "Broadcom Limited"),
        ("torex", "Torex Semicon"),
        // Chinese manufacturers (popular on JLCPCB)
        ("fh", "FH (Guangdong Fenghua Advanced Tech)"),
        ("fenghua", "FH (Guangdong Fenghua Advanced Tech)"),
        ("guangdong fenghua advanced tech", "FH (Guangdong Fenghua Advanced Tech)"),
        ("sxn", "SXN(Shun Xiang Nuo Elec)"),
        ("shun xiang nuo elec", "SXN(Shun Xiang Nuo Elec)"),
        ("changjing", "Jiangsu Changjing Electronics Technology Co., Ltd."),
        ("jscj", "Jiangsu Changjing Electronics Technology Co., Ltd."),
        ("uniroyal", "UNI-ROYAL(Uniroyal Elec)"),
        ("uni-royal", "UNI-ROYAL(Uniroyal Elec)"),
        ("uniroyal elec", "UNI-ROYAL(Uniroyal Elec)"),
        ("mdd", "MDD(Microdiode Semiconductor)"),
        ("microdiode", "MDD(Microdiode Semiconductor)"),
        ("microdiode semiconductor", "MDD(Microdiode Semiconductor)"),
        ("boomele", "BOOMELE(Boom Precision Elec)"),
        ("boom precision elec", "BOOMELE(Boom Precision Elec)"),
        ("stc", "STC Micro"),
        ("kento", "Hubei KENTO Elec"),
        ("hxy", "HXY MOSFET"),
        ("slkor", "Slkor(SLKORMICRO Elec.)"),
        ("slkormicro", "Slkor(SLKORMICRO Elec.)"),
        ("slkormicro elec.", "Slkor(SLKORMICRO Elec.)"),
        ("utc", "UTC(Unisonic Tech)"),
        ("unisonic tech", "UTC(Unisonic Tech)"),
        ("holtek", "Holtek Semicon"),
        // Chinese power/analog IC makers
        ("umw", "UMW(Youtai Semiconductor Co., Ltd.)"),
        ("youtai", "UMW(Youtai Semiconductor Co., Ltd.)"),
        ("youtai semiconductor", "UMW(Youtai Semiconductor Co., Ltd.)"),
        ("youtai semiconductor co., ltd.", "UMW(Youtai Semiconductor Co., Ltd.)"),
        ("sg micro", "SGMICRO"),
        ("xl semiconductor", "XLSEMI"),
        ("nce", "Wuxi NCE Power Semiconductor"),
        ("nce power", "Wuxi NCE Power Semiconductor"),
        ("goford semiconductor", "GOFORD"),
        ("chipown", "Wuxi Chipown Micro-electronics"),
        ("threepeak", "3PEAK"),
        ("microne", "MICRONE(Nanjing Micro One Elec)"),
        ("micro one", "MICRONE(Nanjing Micro One Elec)"),
        ("nanjing micro one elec", "MICRONE(Nanjing Micro One Elec)"),
        ("ams", "Advanced Monolithic Systems"),
        // Chinese transistor/MOSFET makers
        ("aos", "Alpha & Omega Semicon"),
        ("alpha omega", "Alpha & Omega Semicon"),
        ("aosemi", "Alpha & Omega Semicon"),
        ("hottech", "Guangdong Hottech"),
        ("hjc", "hongjiacheng"),
        ("jsm", "JSMSEMI"),
        ("msk", "MSKSEMI"),
        // Chinese MCU makers
        ("gd", "GigaDevice Semicon Beijing"),
        ("gigadevice", "GigaDevice Semicon Beijing"),
        ("wch", "WCH(Jiangsu Qin Heng)"),
        ("qinheng", "WCH(Jiangsu Qin Heng)"),
        ("jiangsu qin heng", "WCH(Jiangsu Qin Heng)"),
        ("ch32", "WCH(Jiangsu Qin Heng)"),
        ("at32", "ARTERY"),
        ("apm", "Geehy"),
        ("apm32", "Geehy"),
        ("py32", "PUYA"),
        ("nuvoton", "Nuvoton Tech"),
        ("padauk", "PADAUK Tech"),
        ("fmd", "FMD(Fremont Micro Devices)"),
        ("fremont", "FMD(Fremont Micro Devices)"),
        ("fremont micro devices", "FMD(Fremont Micro Devices)"),
        ("hk32", "HK"),
        ("cw32", "CW"),
        // Chinese passive/capacitor makers
        ("hkr", "HKR(Hong Kong Resistors)"),
        ("hong kong resistors", "HKR(Hong Kong Resistors)"),
        // Optoelectronics / LEDs
        ("everlight", "Everlight Elec"),
        ("osram", "OSRAM Opto Semicon"),
        ("liteon", "Lite-On"),
        ("sharp", "Sharp Microelectronics"),
        ("ws", "Worldsemi"),
        ("ws2812", "Worldsemi"),
        ("nationstar", "Foshan NationStar Optoelectronics"),
        ("cree", "CREE LED"),
        ("yongyu", "Yongyu Photoelectric"),
        // Connectors
        ("kangnex", "Ningbo Kangnex Elec"),
        ("cjt", "CJT(Changjiang Connectors)"),
        ("changjiang", "CJT(Changjiang Connectors)"),
        ("changjiang connectors", "CJT(Changjiang Connectors)"),
        ("hroparts", "Korean Hroparts Elec"),
        ("korean hroparts", "Korean Hroparts Elec"),
        ("hanrun", "HANRUN(Zhongshan HanRun Elec)"),
        ("zhongshan hanrun elec", "HANRUN(Zhongshan HanRun Elec)"),
        ("shouhan", "SHOU HAN"),
        ("amphenol", "Amphenol ICC"),
        ("te", "TE Connectivity"),
        ("xkb", "XKB Connection"),
        ("kinghelm", "Shenzhen Kinghelm Elec"),
        // Passives
        ("samsung", "Samsung Electro-Mechanics"),
        ("murata", "Murata Electronics"),
        ("tdk", "TDK Corporation"),
        ("wurth", "Wurth Elektronik"),
        ("würth", "Wurth Elektronik"),
        // Crystals / Oscillators
        ("yxc", "YXC Crystal Oscillators"),
        ("yangxing", "YXC Crystal Oscillators"),
        ("seiko epson", "Seiko"),
        ("taitien", "TAITIEN Elec"),
        ("hosonic", "Hosonic Elec"),
        // MCU/SoC
        ("espressif", "Espressif Systems"),
        ("nordic", "Nordic Semicon"),
        ("nordic semiconductor", "Nordic Semicon"),
        ("silabs", "Silicon Labs"),
        ("rpi", "Raspberry Pi"),
        // Memory
        ("winbond", "Winbond Elec"),
        ("micron", "Micron Tech"),
        ("micron technology", "Micron Tech"),
        ("issi", "ISSI(Integrated Silicon Solution)"),
        ("integrated silicon solution", "ISSI(Integrated Silicon Solution)"),
        ("macronix", "MXIC(Macronix)"),
        ("mxic", "MXIC(Macronix)"),
        ("ramxeed", "RAMXEED/FUJITSU"),
        ("fujitsu", "RAMXEED/FUJITSU"),
        // Motor drivers / Power
        ("allegro", "Allegro MicroSystems, LLC"),
        ("allegro microsystems", "Allegro MicroSystems, LLC"),
        ("richtek", "Richtek Tech"),
        // Relays
        ("omron", "Omron Electronics"),
        ("hongfa", "HF(Xiamen Hongfa Electroacoustic)"),
        ("hf", "HF(Xiamen Hongfa Electroacoustic)"),
        ("xiamen hongfa electroacoustic", "HF(Xiamen Hongfa Electroacoustic)"),
        ("songle", "Ningbo Songle Relay"),
        ("hke", "Zhejiang HKE"),
        // Sensors
        ("invensense", "TDK InvenSense"),
        // Chinese IC makers
        ("belling", "BL(Shanghai Belling)"),
        ("shanghai belling", "BL(Shanghai Belling)"),
        ("bl", "BL(Shanghai Belling)"),
        ("on-bright", "OB(On-Bright Elec)"),
        ("onbright", "OB(On-Bright Elec)"),
        ("ob", "OB(On-Bright Elec)"),
        ("on-bright elec", "OB(On-Bright Elec)"),
        ("prisemi", "Shanghai Prisemi Elec"),
        // Battery Management
        ("consonance", "ShangHai Consonance Elec"),
        ("shanghai consonance", "ShangHai Consonance Elec"),
        // Circuit Protection / ESD
        ("ruilon", "RUILON(Shenzhen Ruilongyuan Elec)"),
        ("ruilongyuan", "RUILON(Shenzhen Ruilongyuan Elec)"),
        ("shenzhen ruilongyuan elec", "RUILON(Shenzhen Ruilongyuan Elec)"),
        ("seaward", "Seaward Elec"),
        // Other
        ("semtech", "ST(Semtech)"),
        ("pulse", "Pulse Elec"),
    ])
}
```

- [ ] **Step 2: Verify data fidelity**

Since there's no pytest to port, verify by diffing the transcribed data
against the Python source directly:

```bash
python3 - <<'EOF'
import re
content = open("src/pcbparts_mcp/manufacturer_aliases.py").read()
m = re.search(r"KNOWN_MANUFACTURERS: set\[str\] = \{(.*?)\n\}\n", content, re.S)
names = sorted(re.findall(r'"((?:[^"\\]|\\.)*)"', m.group(1)))
print("python KNOWN_MANUFACTURERS:", len(names))
m = re.search(r"MANUFACTURER_ALIASES: dict\[str, str\] = \{(.*?)\n\}\n", content, re.S)
pairs = sorted(re.findall(r'^\s*"((?:[^"\\]|\\.)*)":\s*"((?:[^"\\]|\\.)*)"', m.group(1), re.M))
print("python MANUFACTURER_ALIASES:", len(pairs))
EOF
```

Compare the counts (142 and 164) and spot-check entries against
`rust/crates/pcbparts-parsers/src/manufacturer_aliases.rs`. (This exact
diff was already run against the code above — 142/142 and 164/164, zero
mismatches.)

- [ ] **Step 3: Confirm the crate still builds clean**

Run: `cargo build -p pcbparts-parsers`
Expected: builds with no errors (data-only module, nothing to test-run).

- [ ] **Step 4: Commit**

```bash
git add rust/crates/pcbparts-parsers/src/manufacturer_aliases.rs
git commit -m "rust: port manufacturer_aliases.py data (no Python tests exist to port)"
```

---

### Task 6: subcategory_aliases.rs

**Files:**
- Create: `rust/crates/pcbparts-parsers/src/subcategory_aliases.rs`

**Interfaces:**
- Produces: `subcategory_aliases() -> HashMap<&'static str, &'static str>`
  (370 entries), `resolve_subcategory_name(name: &str, name_to_id:
  &HashMap<String, i64>, aliases: Option<&HashMap<&str, &str>>) ->
  Option<i64>`, `find_similar_subcategories(name: &str, name_to_id:
  &HashMap<String, i64>, subcategory_info: &HashMap<i64, (String, String)>,
  limit: usize) -> Vec<SimilarSubcategory>` — consumed by Phase 3's search
  engine and Phase 5's component-DB `attributes.rs`/`categories.rs` ports.
- Same as Task 5: no Python test file exists for `subcategory_aliases.py`;
  no Rust tests invented. Data fidelity verified by programmatic diff
  (370/370 entries, zero mismatches).

- [ ] **Step 1: Write the data + functions**

```rust
// rust/crates/pcbparts-parsers/src/subcategory_aliases.rs
use std::collections::HashMap;

pub fn subcategory_aliases() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        // CAPACITORS
        ("capacitor", "multilayer ceramic capacitors mlcc - smd/smt"),
        ("capacitors", "multilayer ceramic capacitors mlcc - smd/smt"),
        ("cap", "multilayer ceramic capacitors mlcc - smd/smt"),
        ("mlcc", "multilayer ceramic capacitors mlcc - smd/smt"),
        ("smd capacitor", "multilayer ceramic capacitors mlcc - smd/smt"),
        ("ceramic capacitor", "multilayer ceramic capacitors mlcc - smd/smt"),
        ("smd ceramic capacitor", "multilayer ceramic capacitors mlcc - smd/smt"),
        ("electrolytic", "aluminum electrolytic capacitors - smd"),
        ("electrolytic capacitor", "aluminum electrolytic capacitors - smd"),
        ("smd electrolytic", "aluminum electrolytic capacitors - smd"),
        ("radial electrolytic", "aluminum electrolytic capacitors - leaded"),
        ("radial electrolytic capacitor", "aluminum electrolytic capacitors - leaded"),
        ("radial capacitor", "aluminum electrolytic capacitors - leaded"),
        ("through hole electrolytic", "aluminum electrolytic capacitors - leaded"),
        ("pth electrolytic", "aluminum electrolytic capacitors - leaded"),
        ("leaded electrolytic", "aluminum electrolytic capacitors - leaded"),
        ("tantalum", "tantalum capacitors"),
        ("tantalum capacitor", "tantalum capacitors"),
        ("film capacitor", "film capacitors"),
        // RESISTORS
        ("resistor", "chip resistor - surface mount"),
        ("resistors", "chip resistor - surface mount"),
        ("smd resistor", "chip resistor - surface mount"),
        ("chip resistor", "chip resistor - surface mount"),
        ("through hole resistor", "through hole resistors"),
        ("tht resistor", "through hole resistors"),
        ("current sense resistor", "current sense resistors / shunt resistors"),
        ("shunt resistor", "current sense resistors / shunt resistors"),
        ("resistor array", "resistor networks, arrays"),
        ("resistor network", "resistor networks, arrays"),
        ("potentiometer", "potentiometers, variable resistors"),
        ("potentiometers", "potentiometers, variable resistors"),
        ("pot", "potentiometers, variable resistors"),
        ("trimmer", "potentiometers, variable resistors"),
        ("trimpot", "potentiometers, variable resistors"),
        ("trim pot", "potentiometers, variable resistors"),
        ("variable resistor", "potentiometers, variable resistors"),
        ("adjustable resistor", "potentiometers, variable resistors"),
        // INDUCTORS
        ("inductor", "inductors (smd)"),
        ("inductors", "inductors (smd)"),
        ("smd inductor", "inductors (smd)"),
        ("power inductor", "power inductors"),
        ("power inductors", "power inductors"),
        ("coil", "inductors (smd)"),
        ("ferrite bead", "ferrite beads"),
        ("ferrite", "ferrite beads"),
        // DIODES
        ("diode", "switching diodes"),
        ("diodes", "switching diodes"),
        ("schottky", "schottky diodes"),
        ("schottky diode", "schottky diodes"),
        ("zener", "zener diodes"),
        ("zener diode", "zener diodes"),
        ("tvs", "esd and surge protection (tvs/esd)"),
        ("tvs diode", "esd and surge protection (tvs/esd)"),
        ("esd diode", "esd and surge protection (tvs/esd)"),
        ("esd protection", "esd and surge protection (tvs/esd)"),
        ("surge protection", "esd and surge protection (tvs/esd)"),
        ("esd", "esd and surge protection (tvs/esd)"),
        ("rectifier", "bridge rectifiers"),
        ("rectifier diode", "diodes - general purpose"),
        ("fast recovery diode", "fast recovery / high efficiency diodes"),
        ("fast diode", "fast recovery / high efficiency diodes"),
        ("frd", "fast recovery / high efficiency diodes"),
        ("sic diode", "sic diodes"),
        ("silicon carbide diode", "sic diodes"),
        // TRANSISTORS - MOSFETs
        ("mosfet", "mosfets"),
        ("mosfets", "mosfets"),
        ("n-channel", "mosfets"),
        ("p-channel", "mosfets"),
        ("n-channel mosfet", "mosfets"),
        ("p-channel mosfet", "mosfets"),
        ("nmos", "mosfets"),
        ("pmos", "mosfets"),
        ("power mosfet", "mosfets"),
        ("gan mosfet", "gan transistors(gan hemt)"),
        ("gan transistor", "gan transistors(gan hemt)"),
        ("gan hemt", "gan transistors(gan hemt)"),
        ("gallium nitride", "gan transistors(gan hemt)"),
        ("sic mosfet", "silicon carbide field effect transistor (mosfet)"),
        ("sic transistor", "silicon carbide field effect transistor (mosfet)"),
        ("silicon carbide mosfet", "silicon carbide field effect transistor (mosfet)"),
        // TRANSISTORS - BJT (actual DB name: "Bipolar (BJT)")
        ("bjt", "bipolar (bjt)"),
        ("transistor", "bipolar (bjt)"),
        ("npn", "bipolar (bjt)"),
        ("pnp", "bipolar (bjt)"),
        ("npn transistor", "bipolar (bjt)"),
        ("pnp transistor", "bipolar (bjt)"),
        // TRANSISTORS - Other types
        ("phototransistor", "phototransistors"),
        ("photo transistor", "phototransistors"),
        ("darlington", "darlington transistors"),
        ("darlington transistor", "darlington transistors"),
        ("jfet", "jfets"),
        ("igbt", "igbt transistors / modules"),
        // CRYSTALS / OSCILLATORS
        ("crystal", "crystals"),
        ("crystals", "crystals"),
        ("xtal", "crystals"),
        ("oscillator", "crystal oscillators"),
        ("tcxo", "temperature compensated crystal oscillators (tcxo)"),
        // CONNECTORS
        ("usb connector", "usb connectors"),
        ("usb-c", "usb connectors"),
        ("usb type-c", "usb connectors"),
        ("type-c", "usb connectors"),
        ("type-c connector", "usb connectors"),
        ("pin header", "pin headers"),
        ("header", "pin headers"),
        ("male header", "pin headers"),
        ("header pin", "pin headers"),
        ("straight header", "pin headers"),
        ("right angle header", "pin headers"),
        ("single row header", "pin headers"),
        ("dual row header", "pin headers"),
        ("double row header", "pin headers"),
        ("female header", "female headers"),
        ("socket header", "female headers"),
        ("receptacle header", "female headers"),
        ("female socket", "female headers"),
        ("ic socket", "ic / transistor socket"),
        ("dip socket", "ic / transistor socket"),
        ("transistor socket", "ic / transistor socket"),
        ("plcc socket", "ic / transistor socket"),
        ("chip socket", "ic / transistor socket"),
        ("jst", "wire to board connector"),
        ("jst connector", "wire to board connector"),
        ("jst sh", "wire to board connector"),
        ("jst ph", "wire to board connector"),
        ("jst xh", "wire to board connector"),
        ("jst zh", "wire to board connector"),
        ("wire to board", "wire to board connector"),
        ("qwiic", "wire to board connector"),
        ("qwiic connector", "wire to board connector"),
        ("stemma qt", "wire to board connector"),
        ("stemma", "wire to board connector"),
        ("easyc", "wire to board connector"),
        ("terminal", "screw terminal blocks"),
        ("terminal block", "screw terminal blocks"),
        ("screw terminal", "screw terminal blocks"),
        ("screw terminal block", "screw terminal blocks"),
        ("pluggable terminal", "pluggable system terminal block"),
        ("pluggable terminal block", "pluggable system terminal block"),
        ("barrier terminal", "barrier terminal blocks"),
        ("barrier terminal block", "barrier terminal blocks"),
        ("spring terminal", "spring clamp system terminal block"),
        ("spring clamp terminal", "spring clamp system terminal block"),
        ("idc", "idc connectors"),
        ("idc connector", "idc connectors"),
        ("ribbon connector", "idc connectors"),
        ("ffc", "ffc, fpc (flat flexible) connector assemblies"),
        ("fpc", "ffc, fpc (flat flexible) connector assemblies"),
        ("ffc connector", "ffc, fpc (flat flexible) connector assemblies"),
        ("fpc connector", "ffc, fpc (flat flexible) connector assemblies"),
        ("flat flex", "ffc, fpc (flat flexible) connector assemblies"),
        ("flat flexible", "ffc, fpc (flat flexible) connector assemblies"),
        ("zif connector", "ffc, fpc (flat flexible) connector assemblies"),
        ("board to board", "board-to-board and backplane connector"),
        ("btb connector", "board-to-board and backplane connector"),
        ("mezzanine connector", "board-to-board and backplane connector"),
        ("sma", "coaxial connectors (rf)"),
        ("sma connector", "coaxial connectors (rf)"),
        ("coax", "coaxial connectors (rf)"),
        ("coaxial", "coaxial connectors (rf)"),
        ("rf connector", "coaxial connectors (rf)"),
        ("u.fl", "coaxial connectors (rf)"),
        ("ipex", "coaxial connectors (rf)"),
        ("ipx", "coaxial connectors (rf)"),
        ("mhf", "coaxial connectors (rf)"),
        ("audio jack", "audio connectors"),
        ("headphone jack", "audio connectors"),
        ("3.5mm jack", "audio connectors"),
        ("phone jack", "audio connectors"),
        ("dc jack", "dc power connectors"),
        ("barrel jack", "dc power connectors"),
        ("dc power jack", "dc power connectors"),
        ("power connector", "dc power connectors"),
        ("rj45", "ethernet connectors / modular connectors (rj45 rj11)"),
        ("rj11", "ethernet connectors / modular connectors (rj45 rj11)"),
        ("ethernet connector", "ethernet connectors / modular connectors (rj45 rj11)"),
        ("ethernet jack", "ethernet connectors / modular connectors (rj45 rj11)"),
        ("modular jack", "ethernet connectors / modular connectors (rj45 rj11)"),
        ("sd card", "sd card / memory card connector"),
        ("sd card connector", "sd card / memory card connector"),
        ("microsd", "sd card / memory card connector"),
        ("micro sd", "sd card / memory card connector"),
        ("tf card", "sd card / memory card connector"),
        ("memory card connector", "sd card / memory card connector"),
        ("sim card", "sim card connectors"),
        ("sim connector", "sim card connectors"),
        ("nano sim", "sim card connectors"),
        ("micro sim", "sim card connectors"),
        ("hdmi", "hdmi connectors"),
        ("hdmi connector", "hdmi connectors"),
        ("mini hdmi", "hdmi connectors"),
        ("micro hdmi", "hdmi connectors"),
        ("d-sub", "d-sub / vga connectors"),
        ("dsub", "d-sub / vga connectors"),
        ("vga", "d-sub / vga connectors"),
        ("vga connector", "d-sub / vga connectors"),
        ("db9", "d-sub / vga connectors"),
        ("db15", "d-sub / vga connectors"),
        ("db25", "d-sub / vga connectors"),
        ("banana plug", "banana connectors / alligator clips"),
        ("banana connector", "banana connectors / alligator clips"),
        ("alligator clip", "banana connectors / alligator clips"),
        ("crocodile clip", "banana connectors / alligator clips"),
        ("pogo pin", "pogo pin spring probe connector"),
        ("spring probe", "pogo pin spring probe connector"),
        ("test probe", "pogo pin spring probe connector"),
        // ICs - VOLTAGE REGULATORS
        ("ldo", "voltage regulators - linear, low drop out (ldo) regulators"),
        ("regulator", "voltage regulators - linear, low drop out (ldo) regulators"),
        ("linear regulator", "voltage regulators - linear, low drop out (ldo) regulators"),
        ("voltage regulator", "voltage regulators - linear, low drop out (ldo) regulators"),
        // ICs - DC-DC CONVERTERS
        ("dc-dc", "dc-dc converters"),
        ("dc dc", "dc-dc converters"),
        ("dc dc converter", "dc-dc converters"),
        ("dc-dc converter", "dc-dc converters"),
        ("buck", "dc-dc converters"),
        ("buck converter", "dc-dc converters"),
        ("boost", "dc-dc converters"),
        ("boost converter", "dc-dc converters"),
        ("buck-boost", "dc-dc converters"),
        // ICs - OP AMPS
        ("op amp", "operational amplifier"),
        ("opamp", "operational amplifier"),
        ("op-amp", "operational amplifier"),
        ("operational amplifier", "operational amplifier"),
        // ICs - DATA CONVERTERS
        ("adc", "analog to digital converters (adc)"),
        ("dac", "digital to analog converters (dac)"),
        // ICs - MICROCONTROLLERS
        ("mcu", "microcontrollers (mcu/mpu/soc)"),
        ("microcontroller", "microcontrollers (mcu/mpu/soc)"),
        // LEDs
        ("led", "led indication - discrete"),
        ("leds", "led indication - discrete"),
        ("smd led", "led indication - discrete"),
        ("indicator led", "led indication - discrete"),
        ("rgb led", "rgb leds"),
        ("addressable led", "rgb leds(built-in ic)"),
        ("ws2812", "rgb leds(built-in ic)"),
        ("neopixel", "rgb leds(built-in ic)"),
        ("ir led", "infrared led emitters"),
        ("infrared led", "infrared led emitters"),
        ("uv led", "ultraviolet leds (uvled)"),
        // SWITCHES
        ("tactile switch", "tactile switches"),
        ("tact switch", "tactile switches"),
        ("push button", "tactile switches"),
        ("pushbutton", "tactile switches"),
        ("button", "tactile switches"),
        ("pushbutton switch", "pushbutton switches"),
        ("panel button", "pushbutton switches"),
        ("dip switch", "dip switches"),
        ("toggle switch", "toggle switches"),
        ("slide switch", "slide switches"),
        ("rocker switch", "rocker switches"),
        // SENSORS
        ("temperature and humidity sensor", "temperature and humidity sensor"),
        ("humidity and temperature sensor", "temperature and humidity sensor"),
        ("temperature humidity sensor", "temperature and humidity sensor"),
        ("humidity temperature sensor", "temperature and humidity sensor"),
        ("temp and humidity sensor", "temperature and humidity sensor"),
        ("humidity and temp sensor", "temperature and humidity sensor"),
        ("temp humidity sensor", "temperature and humidity sensor"),
        ("humidity temp sensor", "temperature and humidity sensor"),
        ("dht sensor", "temperature and humidity sensor"),
        ("sht sensor", "temperature and humidity sensor"),
        ("bme sensor", "temperature and humidity sensor"),
        ("aht sensor", "temperature and humidity sensor"),
        ("temperature sensor", "temperature sensors"),
        ("temp sensor", "temperature sensors"),
        ("thermistor", "ntc thermistors"),
        ("ntc", "ntc thermistors"),
        ("ptc thermistor", "ptc thermistors"),
        ("accelerometer", "accelerometers"),
        ("gyroscope", "accelerometers"),
        ("imu", "accelerometers"),
        ("hall sensor", "linear hall sensors"),
        ("hall effect", "linear hall sensors"),
        ("hall effect sensor", "linear hall sensors"),
        ("hall switch", "hall switches"),
        ("hall effect switch", "hall switches"),
        ("current sensor", "current sensors"),
        ("magnetic sensor", "magnetic angle sensors"),
        ("light sensor", "ambient light sensors"),
        ("ambient light", "ambient light sensors"),
        ("photodiode", "photodiodes"),
        ("photoresistor", "photoresistors"),
        ("ldr", "photoresistors"),
        ("pressure sensor", "pressure sensors"),
        ("gas sensor", "gas sensors"),
        ("proximity sensor", "proximity sensors"),
        ("ultrasonic sensor", "ultrasonic receivers, transmitters"),
        ("encoder", "rotary encoders"),
        ("rotary encoder", "rotary encoders"),
        // ANTENNAS
        ("antenna", "antennas"),
        ("antennas", "antennas"),
        ("ceramic antenna", "antennas"),
        ("chip antenna", "antennas"),
        ("pcb antenna", "antennas"),
        ("external antenna", "antennas"),
        ("2.4ghz antenna", "antennas"),
        ("wifi antenna", "antennas"),
        ("bluetooth antenna", "antennas"),
        ("ble antenna", "antennas"),
        ("gps antenna", "antennas"),
        ("lte antenna", "antennas"),
        ("5g antenna", "antennas"),
        // MODULES
        ("wifi module", "wifi modules"),
        ("bluetooth module", "bluetooth modules"),
        ("ble module", "bluetooth modules"),
        ("lora module", "lora modules"),
        ("gps module", "gnss modules"),
        ("rf module", "rf modules"),
        // BATTERY MANAGEMENT
        ("battery charger", "battery management"),
        ("battery management", "battery management"),
        ("lithium charger", "battery management"),
        ("li-ion charger", "battery management"),
        ("lipo charger", "battery management"),
        ("charge controller", "battery management"),
        ("bms", "battery management"),
        // POWER MANAGEMENT
        ("power switch", "power distribution switches"),
        ("load switch", "power distribution switches"),
        ("hot swap", "power distribution switches"),
        // FUSES
        ("fuse", "disposable fuses"),
        ("resettable fuse", "resettable fuses"),
        ("ptc fuse", "resettable fuses"),
        ("polyfuse", "resettable fuses"),
        // OPTOCOUPLERS
        ("optocoupler", "transistor, photovoltaic output optoisolators"),
        ("optoisolator", "transistor, photovoltaic output optoisolators"),
        ("opto", "transistor, photovoltaic output optoisolators"),
        // MOTOR DRIVERS
        ("motor driver", "motor driver ics"),
        ("h-bridge", "motor driver ics"),
        ("stepper driver", "motor driver ics"),
        // RELAYS
        ("relay", "signal relays"),
        ("solid state relay", "solid state relays"),
        ("ssr", "solid state relays"),
        // TIMING
        ("555 timer", "555 timers / counters"),
        ("timer", "555 timers / counters"),
        ("rtc", "real time clocks"),
        ("real time clock", "real time clocks"),
        // MEMORY
        ("eeprom", "eeprom"),
        ("flash", "nor flash"),
        ("nor flash", "nor flash"),
        ("nand", "nand flash"),
        ("nand flash", "nand flash"),
        ("sram", "sram"),
        ("fram", "fram"),
        // AUDIO
        ("audio amplifier", "audio amplifiers"),
        ("class d", "audio amplifiers"),
        ("class d amplifier", "audio amplifiers"),
        ("codec", "audio interface ics"),
        ("audio codec", "audio interface ics"),
        ("buzzer", "buzzers"),
        ("speaker", "speakers"),
        ("microphone", "microphones"),
        // DISPLAYS
        ("7 segment", "led segment displays"),
        ("seven segment", "led segment displays"),
        ("segment display", "led segment displays"),
        ("lcd", "lcd screen"),
        ("lcd display", "lcd screen"),
        ("oled", "oled display"),
        ("oled display", "oled display"),
        ("tft", "lcd screen"),
        ("tft lcd", "lcd screen"),
        // INTERFACE ICs
        ("level shifter", "translators, level shifters"),
        ("voltage translator", "translators, level shifters"),
        ("uart", "uart"),
        ("usb uart", "usb converters"),
        ("uart to usb", "usb converters"),
        ("usb to uart", "usb converters"),
        ("usb serial", "usb converters"),
        ("usb to serial", "usb converters"),
        ("serial to usb", "usb converters"),
        ("usb converter", "usb converters"),
        // CURRENT SENSE AMPLIFIERS
        ("current sense amplifier", "current sense amplifiers"),
        ("current sense amp", "current sense amplifiers"),
        ("current monitor", "current sense amplifiers"),
        ("power monitor", "current sense amplifiers"),
    ])
}

/// Resolve subcategory name to ID. Case-insensitive, supports aliases and
/// partial match.
///
/// Matching priority:
/// 1. Common alias (e.g., "MLCC" -> "Multilayer Ceramic Capacitors MLCC - SMD/SMT")
/// 2. Exact match (e.g., "crystals" -> "crystals")
/// 3. Shortest containing match (e.g., "crystal" -> "crystals" not "crystal oscillators")
pub fn resolve_subcategory_name(
    name: &str,
    name_to_id: &HashMap<String, i64>,
    aliases: Option<&HashMap<&str, &str>>,
) -> Option<i64> {
    if name.is_empty() {
        return None;
    }

    let owned_aliases;
    let aliases: &HashMap<&str, &str> = match aliases {
        Some(a) => a,
        None => {
            owned_aliases = subcategory_aliases();
            &owned_aliases
        }
    };

    let name_lower = name.to_lowercase();

    if let Some(alias_target) = aliases.get(name_lower.as_str()) {
        if let Some(id) = name_to_id.get(*alias_target) {
            return Some(*id);
        }
    }

    if let Some(id) = name_to_id.get(name_lower.as_str()) {
        return Some(*id);
    }

    let mut matches: Vec<(&str, i64)> = name_to_id
        .iter()
        .filter(|(k, _)| k.contains(&name_lower))
        .map(|(k, v)| (k.as_str(), *v))
        .collect();

    if matches.is_empty() {
        return None;
    }

    matches.sort_by_key(|(k, _)| k.len());
    Some(matches[0].1)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SimilarSubcategory {
    pub id: i64,
    pub name: String,
    pub category: String,
}

/// Find subcategories similar to the given name (for error suggestions).
pub fn find_similar_subcategories(
    name: &str,
    name_to_id: &HashMap<String, i64>,
    subcategory_info: &HashMap<i64, (String, String)>, // id -> (name, category_name)
    limit: usize,
) -> Vec<SimilarSubcategory> {
    let name_lower = name.to_lowercase();
    let words: Vec<&str> = name_lower.split_whitespace().collect();

    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut unique: Vec<SimilarSubcategory> = Vec::new();

    // Iterate name_to_id in a stable order (sorted by key) so results are
    // deterministic — Python dict iteration is insertion-ordered, which a
    // HashMap doesn't preserve; callers that need Python's exact ordering
    // should pass an ordered structure. Sorted-by-key is a reasonable,
    // deterministic stand-in documented here for the plan reviewer.
    let mut entries: Vec<(&String, &i64)> = name_to_id.iter().collect();
    entries.sort_by_key(|(k, _)| k.as_str());

    'outer: for (subcat_name_lower, subcat_id) in entries {
        for word in &words {
            if word.len() >= 3 && subcat_name_lower.contains(word) {
                if seen.insert(*subcat_id) {
                    let (nm, cat) = subcategory_info
                        .get(subcat_id)
                        .cloned()
                        .unwrap_or((subcat_name_lower.clone(), String::new()));
                    unique.push(SimilarSubcategory {
                        id: *subcat_id,
                        name: nm,
                        category: cat,
                    });
                    if unique.len() >= limit {
                        break 'outer;
                    }
                }
                break;
            }
        }
    }

    unique
}
```

- [ ] **Step 2: Verify data fidelity**

Same diff approach as Task 5, against `SUBCATEGORY_ALIASES` in
`src/pcbparts_mcp/subcategory_aliases.py` (370 entries — already verified,
zero mismatches).

- [ ] **Step 3: Confirm the crate builds clean**

Run: `cargo build -p pcbparts-parsers`
Expected: builds with no errors.

- [ ] **Step 4: Run the full crate test suite**

Run: `cd rust && cargo test -p pcbparts-parsers`
Expected: PASS — **71 tests total** (32 parsers + 15 mounting + 9 pinout +
15 design_rules; `manufacturer_aliases`/`subcategory_aliases` contribute
data + functions with no tests, matching Python).

- [ ] **Step 5: Commit**

```bash
git add rust/crates/pcbparts-parsers/src/subcategory_aliases.rs
git commit -m "rust: port subcategory_aliases.py data + resolve/similar functions"
```

## Self-Review Notes

- **Spec coverage:** Every module in the spec's true Phase 2A scope
  (`parsers.py`, `mounting.py`, `manufacturer_aliases.py`,
  `subcategory_aliases.py`, `pinout.py`, `design_rules.py`) has a task.
  `alternatives.py` is explicitly out of scope (Phase 2B, separate plan).
- **No placeholders:** every code block is verbatim, previously
  compiled-and-tested Rust code (71/71 module-specific tests passing, plus
  all 134 Phase 1 tests still green in the same crate/workspace).
- **Type consistency:** `detect_mounting_type`, `parse_easyeda_pins`/`Pin`,
  `get_design_rules`, `resolve_subcategory_name`/`find_similar_subcategories`
  signatures are identical everywhere they're referenced across tasks.
- **Data fidelity:** `manufacturer_aliases.rs` and `subcategory_aliases.rs`
  were verified against their Python sources by programmatic diff (142/142,
  164/164, 370/370 entries, zero mismatches) rather than by test — the
  right verification method given neither Python module has a test file.

## Next Step

**Plan 2B** (a separate document) covers `alternatives.py` —
`SPEC_PARSERS`, `COMPATIBILITY_RULES`, and the compatibility-checking/
scoring logic — which depends on this plan's `parsers.rs`.
