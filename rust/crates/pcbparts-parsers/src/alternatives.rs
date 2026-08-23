use crate::parsers::*;
use serde_json;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy)]
pub enum SpecParser {
    Parser(fn(&str) -> Option<f64>),
    Special,
    StringMatch,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    Higher,
    Lower,
}

pub struct CompatRule {
    pub primary: &'static str,
    pub must_match: &'static [&'static str],
    pub same_or_better: &'static [(&'static str, Direction)],
}

// === Generated from the live SPEC_PARSERS/COMPATIBILITY_RULES/*_SPECS dicts ===
// (verified: 119/119, 123/123, 3/3, 45/45, 5/5 entries match the Python source
// exactly — generated programmatically from a JSON dump of the real objects,
// not hand-transcribed, eliminating transcription risk for this much data.)

pub fn spec_parsers() -> HashMap<&'static str, SpecParser> {
    HashMap::from([
        ("Average Rectified Current", SpecParser::Parser(parse_current)),
        ("B Constant (25℃/100℃)", SpecParser::StringMatch),
        ("Capacitance", SpecParser::Parser(parse_capacitance)),
        ("Cell Resistance @ Illuminance", SpecParser::Parser(parse_resistance)),
        ("Channel Type", SpecParser::StringMatch),
        ("Charge Current - Max", SpecParser::Parser(parse_current)),
        ("Circuit", SpecParser::StringMatch),
        ("Clamping Voltage", SpecParser::Parser(parse_voltage)),
        ("Class", SpecParser::StringMatch),
        ("Coil Voltage", SpecParser::Parser(parse_voltage)),
        ("Collector - Emitter Voltage VCEO", SpecParser::Parser(parse_voltage)),
        ("Collector-Emitter Breakdown Voltage (Vces)", SpecParser::Parser(parse_voltage)),
        ("Color", SpecParser::StringMatch),
        ("Connector Type", SpecParser::StringMatch),
        ("Contact Current", SpecParser::Parser(parse_current)),
        ("Contact Form", SpecParser::StringMatch),
        ("Contact Rating", SpecParser::Parser(parse_current)),
        ("Current - Average Rectified", SpecParser::Parser(parse_current)),
        ("Current - Collector(Ic)", SpecParser::Parser(parse_current)),
        ("Current - Continuous Drain(Id)", SpecParser::Parser(parse_current)),
        ("Current - Rectified", SpecParser::Parser(parse_current)),
        ("Current - Saturation (Isat)", SpecParser::Parser(parse_current)),
        ("Current - Saturation(Isat)", SpecParser::Parser(parse_current)),
        ("Current Rating", SpecParser::Parser(parse_current)),
        ("Current Rating (Max)", SpecParser::Parser(parse_current)),
        ("DC Resistance(DCR)", SpecParser::Parser(parse_resistance)),
        ("Data Rate", SpecParser::StringMatch),
        ("Data Rate(Max)", SpecParser::StringMatch),
        ("Diameter", SpecParser::Parser(parse_length_mm)),
        ("Direction", SpecParser::StringMatch),
        ("Drain Current (Idss)", SpecParser::Parser(parse_current)),
        ("Drain to Source Voltage", SpecParser::Parser(parse_voltage)),
        ("Driver Circuitry", SpecParser::StringMatch),
        ("Encoder Type", SpecParser::StringMatch),
        ("Energy", SpecParser::StringMatch),
        ("FET Type", SpecParser::StringMatch),
        ("Frequency", SpecParser::Parser(parse_frequency)),
        ("Frequency Stability", SpecParser::Parser(parse_ppm)),
        ("Gate Threshold Voltage", SpecParser::Parser(parse_voltage)),
        ("Gate Threshold Voltage (Vgs(th))", SpecParser::Parser(parse_voltage)),
        ("Gender", SpecParser::StringMatch),
        ("Height", SpecParser::Parser(parse_length_mm)),
        ("Height - Seated (Max)", SpecParser::Parser(parse_length_mm)),
        ("Hold Current", SpecParser::Parser(parse_current)),
        ("Illumination Color", SpecParser::StringMatch),
        ("Impedance", SpecParser::StringMatch),
        ("Impedance @ Frequency", SpecParser::Special),
        ("Impulse Discharge Current", SpecParser::Parser(parse_current)),
        ("Inductance", SpecParser::Parser(parse_inductance)),
        ("Isolation Voltage(Vrms)", SpecParser::Parser(parse_voltage)),
        ("Load Capacitance", SpecParser::Parser(parse_capacitance)),
        ("Load Current", SpecParser::Parser(parse_current)),
        ("Load Voltage", SpecParser::Parser(parse_voltage)),
        ("Mounting Type", SpecParser::StringMatch),
        ("Number of Capacitors", SpecParser::StringMatch),
        ("Number of Cells", SpecParser::StringMatch),
        ("Number of Coils", SpecParser::StringMatch),
        ("Number of Forward Channels", SpecParser::StringMatch),
        ("Number of H-bridges", SpecParser::StringMatch),
        ("Number of Lines", SpecParser::StringMatch),
        ("Number of Pins", SpecParser::StringMatch),
        ("Number of Poles", SpecParser::StringMatch),
        ("Number of Poles Per Deck", SpecParser::StringMatch),
        ("Number of Positions", SpecParser::StringMatch),
        ("Number of Positions or Pins", SpecParser::StringMatch),
        ("Number of Resistors", SpecParser::StringMatch),
        ("Number of Reverse Channels", SpecParser::StringMatch),
        ("Number of Rows", SpecParser::StringMatch),
        ("Number of Segments", SpecParser::StringMatch),
        ("Number of Turns", SpecParser::StringMatch),
        ("Output Current", SpecParser::Parser(parse_current)),
        ("Output Current(Max)", SpecParser::Parser(parse_current)),
        ("Output Power", SpecParser::Parser(parse_power)),
        ("Output Type", SpecParser::StringMatch),
        ("Output Voltage", SpecParser::Parser(parse_voltage)),
        ("Pd - Power Dissipation", SpecParser::Parser(parse_power)),
        ("Peak Pulse Current-Ipp (10/1000us)", SpecParser::Parser(parse_current)),
        ("Peak Pulse Power", SpecParser::Parser(parse_power)),
        ("Peak Wavelength", SpecParser::StringMatch),
        ("Peak off - state voltage(Vdrm)", SpecParser::Parser(parse_voltage)),
        ("Pins Structure", SpecParser::StringMatch),
        ("Pitch", SpecParser::StringMatch),
        ("Positions", SpecParser::StringMatch),
        ("Power(Watts)", SpecParser::Parser(parse_power)),
        ("RDS(on)", SpecParser::Parser(parse_resistance)),
        ("Rated Functioning Temperature", SpecParser::StringMatch),
        ("Rated Power", SpecParser::Parser(parse_power)),
        ("Rated Voltage (Max)", SpecParser::Parser(parse_voltage)),
        ("Ratings", SpecParser::StringMatch),
        ("Resistance", SpecParser::Parser(parse_resistance)),
        ("Resistance @ 25℃", SpecParser::Parser(parse_resistance)),
        ("Reverse Stand-Off Voltage (Vrwm)", SpecParser::Parser(parse_voltage)),
        ("Reverse Voltage", SpecParser::Parser(parse_voltage)),
        ("Self Lock / No Lock", SpecParser::StringMatch),
        ("Sound Pressure Level", SpecParser::Parser(parse_decibels)),
        ("Switching Current(Max)", SpecParser::Parser(parse_current)),
        ("Switching Voltage(Max)", SpecParser::Parser(parse_voltage)),
        ("Temperature Coefficient", SpecParser::StringMatch),
        ("Tolerance", SpecParser::Parser(parse_tolerance)),
        ("Trigger Voltage", SpecParser::Parser(parse_voltage)),
        ("Trip Current", SpecParser::Parser(parse_current)),
        ("Type", SpecParser::StringMatch),
        ("Type of Battery", SpecParser::StringMatch),
        ("Varistor Voltage", SpecParser::Parser(parse_voltage)),
        ("Vce Saturation(VCE(sat))", SpecParser::Parser(parse_voltage)),
        ("Voltage - DC Reverse(Vr)", SpecParser::Parser(parse_voltage)),
        ("Voltage - DC Spark Over", SpecParser::Parser(parse_voltage)),
        ("Voltage - Forward(Vf@If)", SpecParser::Parser(parse_forward_voltage)),
        ("Voltage - Max", SpecParser::Parser(parse_voltage)),
        ("Voltage - Supply", SpecParser::Parser(parse_voltage)),
        ("Voltage Dropout", SpecParser::Parser(parse_voltage)),
        ("Voltage Rating", SpecParser::Parser(parse_voltage)),
        ("Voltage Rating (AC)", SpecParser::Parser(parse_voltage)),
        ("Voltage Rating (DC)", SpecParser::Parser(parse_voltage)),
        ("Voltage Rating (Max)", SpecParser::Parser(parse_voltage)),
        ("Voltage Rating - DC", SpecParser::Parser(parse_voltage)),
        ("Voltage(AC)", SpecParser::Parser(parse_voltage)),
        ("Zener Voltage(Nom)", SpecParser::Parser(parse_voltage)),
        ("type", SpecParser::StringMatch),
    ])
}

pub fn compatibility_rules() -> HashMap<&'static str, CompatRule> {
    HashMap::from([
        ("Aluminum Electrolytic Capacitors (Can - Screw Terminals)", CompatRule { primary: "Capacitance", must_match: &[], same_or_better: &[("Voltage Rating", Direction::Higher)] }),
        ("Aluminum Electrolytic Capacitors - Leaded", CompatRule { primary: "Capacitance", must_match: &[], same_or_better: &[("Voltage Rating", Direction::Higher)] }),
        ("Aluminum Electrolytic Capacitors - SMD", CompatRule { primary: "Capacitance", must_match: &[], same_or_better: &[("Voltage Rating", Direction::Higher)] }),
        ("Audio Amplifiers", CompatRule { primary: "Class", must_match: &["Class"], same_or_better: &[("Output Power", Direction::Higher)] }),
        ("Audio Connectors", CompatRule { primary: "Connector Type", must_match: &["Connector Type"], same_or_better: &[("Current Rating", Direction::Higher), ("Voltage Rating", Direction::Higher)] }),
        ("Automotive Fuses", CompatRule { primary: "Current Rating", must_match: &["Current Rating", "Type"], same_or_better: &[("Voltage Rating (DC)", Direction::Higher)] }),
        ("Automotive Relays", CompatRule { primary: "Coil Voltage", must_match: &["Coil Voltage", "Contact Form"], same_or_better: &[("Contact Rating", Direction::Higher), ("Switching Voltage(Max)", Direction::Higher)] }),
        ("Avalanche Diodes", CompatRule { primary: "Voltage - DC Reverse(Vr)", must_match: &[], same_or_better: &[("Current - Rectified", Direction::Higher), ("Voltage - DC Reverse(Vr)", Direction::Higher)] }),
        ("Barrier Terminal Blocks", CompatRule { primary: "Number of Positions or Pins", must_match: &["Pitch", "Number of Positions or Pins"], same_or_better: &[("Current Rating", Direction::Higher), ("Voltage Rating (Max)", Direction::Higher)] }),
        ("Battery Management", CompatRule { primary: "Type of Battery", must_match: &["Type of Battery", "Number of Cells"], same_or_better: &[("Charge Current - Max", Direction::Higher)] }),
        ("Bipolar (BJT)", CompatRule { primary: "type", must_match: &["type"], same_or_better: &[("Collector - Emitter Voltage VCEO", Direction::Higher), ("Current - Collector(Ic)", Direction::Higher)] }),
        ("Bluetooth Modules", CompatRule { primary: "Voltage - Supply", must_match: &["Voltage - Supply"], same_or_better: &[("Output Power", Direction::Higher)] }),
        ("Bridge Rectifiers", CompatRule { primary: "Voltage - DC Reverse(Vr)", must_match: &[], same_or_better: &[("Current - Rectified", Direction::Higher), ("Voltage - DC Reverse(Vr)", Direction::Higher), ("Voltage - Forward(Vf@If)", Direction::Lower)] }),
        ("Brushed DC Motor Drivers", CompatRule { primary: "Output Current", must_match: &["Number of H-bridges"], same_or_better: &[("Output Current", Direction::Higher), ("Peak Current", Direction::Higher), ("RDS(on)", Direction::Lower)] }),
        ("Buzzers", CompatRule { primary: "Voltage - Supply", must_match: &["Driver Circuitry"], same_or_better: &[("Sound Pressure Level", Direction::Higher)] }),
        ("Capacitor Networks, Arrays", CompatRule { primary: "Capacitance", must_match: &["Number of Capacitors"], same_or_better: &[("Voltage Rating", Direction::Higher)] }),
        ("Ceramic Resonators", CompatRule { primary: "Frequency", must_match: &["Frequency"], same_or_better: &[] }),
        ("Chip Resistor - Surface Mount", CompatRule { primary: "Resistance", must_match: &[], same_or_better: &[("Power(Watts)", Direction::Higher), ("Tolerance", Direction::Lower)] }),
        ("Circular Connectors & Cable Connectors", CompatRule { primary: "Number of Pins", must_match: &["Number of Pins", "Gender"], same_or_better: &[("Current Rating", Direction::Higher), ("Voltage Rating", Direction::Higher)] }),
        ("Coaxial Connectors (RF)", CompatRule { primary: "Connector Type", must_match: &["Connector Type", "Impedance"], same_or_better: &[] }),
        ("Color Ring Inductors / Through Hole Inductors", CompatRule { primary: "Inductance", must_match: &[], same_or_better: &[("Current Rating", Direction::Higher), ("DC Resistance(DCR)", Direction::Lower)] }),
        ("Common Mode Filters", CompatRule { primary: "Impedance @ Frequency", must_match: &["Number of Lines"], same_or_better: &[("Current Rating", Direction::Higher), ("Voltage Rating - DC", Direction::Higher)] }),
        ("Crystal Oscillators", CompatRule { primary: "Frequency", must_match: &["Frequency", "Output Type"], same_or_better: &[("Frequency Stability", Direction::Lower)] }),
        ("Crystals", CompatRule { primary: "Frequency", must_match: &["Frequency", "Load Capacitance"], same_or_better: &[("Frequency Stability", Direction::Lower)] }),
        ("Current Sense Resistors / Shunt Resistors", CompatRule { primary: "Resistance", must_match: &[], same_or_better: &[("Power(Watts)", Direction::Higher), ("Tolerance", Direction::Lower)] }),
        ("DC-DC Converters", CompatRule { primary: "Output Voltage", must_match: &["Topology", "Output Voltage"], same_or_better: &[("Output Current", Direction::Higher)] }),
        ("DIN41612 Connectors", CompatRule { primary: "Number of Pins", must_match: &["Pitch", "Number of Pins", "Number of Rows"], same_or_better: &[("Current Rating", Direction::Higher)] }),
        ("DIP Switches", CompatRule { primary: "Number of Positions", must_match: &["Number of Positions", "Type"], same_or_better: &[("Current Rating", Direction::Higher), ("Voltage Rating", Direction::Higher)] }),
        ("Darlington Transistors", CompatRule { primary: "Type", must_match: &["Type"], same_or_better: &[("Collector - Emitter Voltage VCEO", Direction::Higher), ("Current - Collector(Ic)", Direction::Higher)] }),
        ("Digital Isolators", CompatRule { primary: "Number of Forward Channels", must_match: &["Number of Forward Channels", "Number of Reverse Channels"], same_or_better: &[("Data Rate(Max)", Direction::Higher), ("Isolation Voltage(Vrms)", Direction::Higher)] }),
        ("Digital Transistors", CompatRule { primary: "type", must_match: &["type"], same_or_better: &[("Collector - Emitter Voltage VCEO", Direction::Higher)] }),
        ("Diodes - General Purpose", CompatRule { primary: "Voltage - DC Reverse(Vr)", must_match: &[], same_or_better: &[("Current - Rectified", Direction::Higher), ("Voltage - DC Reverse(Vr)", Direction::Higher)] }),
        ("Diodes - Rectifiers - Fast Recovery", CompatRule { primary: "Voltage - DC Reverse(Vr)", must_match: &[], same_or_better: &[("Current - Average Rectified", Direction::Higher), ("Voltage - DC Reverse(Vr)", Direction::Higher)] }),
        ("DisplayPort (DP) Connector", CompatRule { primary: "Connector Type", must_match: &["Connector Type"], same_or_better: &[] }),
        ("Disposable fuses", CompatRule { primary: "Current Rating", must_match: &["Current Rating", "Type"], same_or_better: &[("Voltage Rating (AC)", Direction::Higher)] }),
        ("ESD and Surge Protection (TVS/ESD)", CompatRule { primary: "Reverse Stand-Off Voltage (Vrwm)", must_match: &[], same_or_better: &[("Clamping Voltage", Direction::Lower), ("Peak Pulse Power", Direction::Higher)] }),
        ("Fast Recovery / High Efficiency Diodes", CompatRule { primary: "Voltage - DC Reverse(Vr)", must_match: &[], same_or_better: &[("Current - Rectified", Direction::Higher), ("Voltage - DC Reverse(Vr)", Direction::Higher)] }),
        ("Female Headers", CompatRule { primary: "Pitch", must_match: &["Pitch", "Number of Positions", "Number of Rows"], same_or_better: &[("Current Rating", Direction::Higher)] }),
        ("Ferrite Beads", CompatRule { primary: "Impedance @ Frequency", must_match: &[], same_or_better: &[("Current Rating", Direction::Higher), ("DC Resistance(DCR)", Direction::Lower)] }),
        ("Film Capacitors", CompatRule { primary: "Capacitance", must_match: &[], same_or_better: &[("Tolerance", Direction::Lower), ("Voltage Rating", Direction::Higher)] }),
        ("Gas Discharge Tube Arresters (GDT)", CompatRule { primary: "Voltage - DC Spark Over", must_match: &["Number of Poles"], same_or_better: &[("Impulse Discharge Current", Direction::Higher)] }),
        ("Gate Drive Optocoupler", CompatRule { primary: "Isolation Voltage(Vrms)", must_match: &[], same_or_better: &[("Isolation Voltage(Vrms)", Direction::Higher), ("Output Current(Max)", Direction::Higher)] }),
        ("HDMI Connectors", CompatRule { primary: "Connector Type", must_match: &["Connector Type", "Gender"], same_or_better: &[] }),
        ("High Effic Rectifier", CompatRule { primary: "Reverse Voltage", must_match: &[], same_or_better: &[("Average Rectified Current", Direction::Higher), ("Reverse Voltage", Direction::Higher)] }),
        ("Horn-Type Electrolytic Capacitors", CompatRule { primary: "Capacitance", must_match: &[], same_or_better: &[("Voltage Rating", Direction::Higher)] }),
        ("Hybrid Aluminum Electrolytic Capacitors", CompatRule { primary: "Capacitance", must_match: &[], same_or_better: &[("Voltage Rating", Direction::Higher)] }),
        ("IDC Connectors", CompatRule { primary: "Number of Positions or Pins", must_match: &["Number of Positions or Pins", "Pitch"], same_or_better: &[("Current Rating", Direction::Higher)] }),
        ("IGBT Transistors / Modules", CompatRule { primary: "Collector-Emitter Breakdown Voltage (Vces)", must_match: &[], same_or_better: &[("Collector-Emitter Breakdown Voltage (Vces)", Direction::Higher), ("Current - Collector(Ic)", Direction::Higher), ("Vce Saturation(VCE(sat))", Direction::Lower)] }),
        ("Inductors (SMD)", CompatRule { primary: "Inductance", must_match: &[], same_or_better: &[("Current - Saturation (Isat)", Direction::Higher), ("Current Rating", Direction::Higher), ("DC Resistance(DCR)", Direction::Lower)] }),
        ("Infrared (IR) LEDs", CompatRule { primary: "Peak Wavelength", must_match: &["Peak Wavelength"], same_or_better: &[] }),
        ("JFETs", CompatRule { primary: "FET Type", must_match: &["FET Type"], same_or_better: &[("Drain Current (Idss)", Direction::Higher), ("RDS(on)", Direction::Lower)] }),
        ("LED - High Brightness", CompatRule { primary: "Illumination Color", must_match: &["Illumination Color"], same_or_better: &[] }),
        ("LED Indication - Discrete", CompatRule { primary: "Illumination Color", must_match: &["Illumination Color"], same_or_better: &[] }),
        ("LED Protection", CompatRule { primary: "Trigger Voltage", must_match: &["Trigger Voltage"], same_or_better: &[("Hold Current", Direction::Higher)] }),
        ("Light Bars, Arrays", CompatRule { primary: "Color", must_match: &["Color", "Number of Segments"], same_or_better: &[] }),
        ("LoRa Modules", CompatRule { primary: "Frequency", must_match: &["Frequency", "Voltage - Supply"], same_or_better: &[("Output Power", Direction::Higher)] }),
        ("Logic Output Optoisolators", CompatRule { primary: "Isolation Voltage(Vrms)", must_match: &[], same_or_better: &[("Data Rate", Direction::Higher), ("Isolation Voltage(Vrms)", Direction::Higher)] }),
        ("MEMS Microphones", CompatRule { primary: "Output Type", must_match: &["Output Type"], same_or_better: &[] }),
        ("MOSFETs", CompatRule { primary: "Drain to Source Voltage", must_match: &[], same_or_better: &[("Current - Continuous Drain(Id)", Direction::Higher), ("Drain to Source Voltage", Direction::Higher), ("RDS(on)", Direction::Lower)] }),
        ("Mica and PTFE Capacitors", CompatRule { primary: "Capacitance", must_match: &[], same_or_better: &[("Tolerance", Direction::Lower), ("Voltage Rating", Direction::Higher)] }),
        ("Microphones", CompatRule { primary: "Direction", must_match: &["Direction"], same_or_better: &[] }),
        ("Motor Driver ICs", CompatRule { primary: "Output Current(Max)", must_match: &[], same_or_better: &[("Output Current", Direction::Higher), ("Output Current(Max)", Direction::Higher), ("Voltage - Supply", Direction::Higher)] }),
        ("Multilayer Ceramic Capacitors MLCC - Leaded", CompatRule { primary: "Capacitance", must_match: &["Temperature Coefficient"], same_or_better: &[("Tolerance", Direction::Lower), ("Voltage Rating", Direction::Higher)] }),
        ("Multilayer Ceramic Capacitors MLCC - SMD/SMT", CompatRule { primary: "Capacitance", must_match: &["Temperature Coefficient"], same_or_better: &[("Tolerance", Direction::Lower), ("Voltage Rating", Direction::Higher)] }),
        ("NTC Thermistors", CompatRule { primary: "Resistance @ 25℃", must_match: &["Resistance @ 25℃", "B Constant (25℃/100℃)"], same_or_better: &[] }),
        ("Niobium Oxide Capacitors", CompatRule { primary: "Capacitance", must_match: &[], same_or_better: &[("Tolerance", Direction::Lower), ("Voltage Rating", Direction::Higher)] }),
        ("Oven Controlled Crystal Oscillators (OCXOs)", CompatRule { primary: "Frequency", must_match: &["Frequency", "Output Type"], same_or_better: &[("Frequency Stability", Direction::Lower)] }),
        ("PTC Thermistors", CompatRule { primary: "Resistance @ 25℃", must_match: &["Resistance @ 25℃"], same_or_better: &[] }),
        ("Photointerrupters - Slot Type - Transistor Output", CompatRule { primary: "Peak Wavelength", must_match: &["Peak Wavelength"], same_or_better: &[("Load Voltage", Direction::Higher), ("Output Current", Direction::Higher)] }),
        ("Photoresistors", CompatRule { primary: "Cell Resistance @ Illuminance", must_match: &[], same_or_better: &[("Voltage - Max", Direction::Higher)] }),
        ("Phototransistors", CompatRule { primary: "Peak Wavelength", must_match: &["Peak Wavelength"], same_or_better: &[("Collector - Emitter Voltage VCEO", Direction::Higher), ("Current - Collector(Ic)", Direction::Higher)] }),
        ("Pin Headers", CompatRule { primary: "Pitch", must_match: &["Pitch", "Number of Pins", "Number of Rows"], same_or_better: &[("Current Rating", Direction::Higher)] }),
        ("Pluggable System Terminal Block", CompatRule { primary: "Number of Positions or Pins", must_match: &["Pitch", "Number of Positions or Pins"], same_or_better: &[("Current Rating", Direction::Higher), ("Voltage Rating (Max)", Direction::Higher)] }),
        ("Polymer Aluminum Capacitors", CompatRule { primary: "Capacitance", must_match: &[], same_or_better: &[("Voltage Rating", Direction::Higher)] }),
        ("Polypropylene Film Capacitors (CBB)", CompatRule { primary: "Capacitance", must_match: &[], same_or_better: &[("Tolerance", Direction::Lower), ("Voltage Rating", Direction::Higher)] }),
        ("Potentiometers, Variable Resistors", CompatRule { primary: "Resistance", must_match: &["Number of Turns"], same_or_better: &[("Power(Watts)", Direction::Higher), ("Tolerance", Direction::Lower)] }),
        ("Power Inductors", CompatRule { primary: "Inductance", must_match: &[], same_or_better: &[("Current - Saturation(Isat)", Direction::Higher), ("Current Rating", Direction::Higher), ("DC Resistance(DCR)", Direction::Lower)] }),
        ("Power Relays", CompatRule { primary: "Coil Voltage", must_match: &["Coil Voltage", "Contact Form"], same_or_better: &[("Contact Rating", Direction::Higher), ("Switching Voltage(Max)", Direction::Higher)] }),
        ("Pushbutton Switches", CompatRule { primary: "Self Lock / No Lock", must_match: &["Self Lock / No Lock"], same_or_better: &[("Contact Current", Direction::Higher), ("Voltage Rating", Direction::Higher)] }),
        ("Reed Relays", CompatRule { primary: "Coil Voltage", must_match: &["Coil Voltage", "Contact Form"], same_or_better: &[("Switching Current(Max)", Direction::Higher), ("Switching Voltage(Max)", Direction::Higher)] }),
        ("Reflective Optical Interrupters", CompatRule { primary: "Output Type", must_match: &["Output Type"], same_or_better: &[("Current - Collector(Ic)", Direction::Higher)] }),
        ("Resettable Fuses", CompatRule { primary: "Hold Current", must_match: &["Hold Current", "Trip Current"], same_or_better: &[("Voltage - Max", Direction::Higher)] }),
        ("Resistor Networks, Arrays", CompatRule { primary: "Resistance", must_match: &["Number of Resistors"], same_or_better: &[("Power(Watts)", Direction::Higher), ("Tolerance", Direction::Lower)] }),
        ("Rocker Switches", CompatRule { primary: "Circuit", must_match: &["Circuit"], same_or_better: &[("Current Rating", Direction::Higher), ("Voltage Rating (DC)", Direction::Higher)] }),
        ("Rotary Encoders", CompatRule { primary: "Encoder Type", must_match: &["Encoder Type"], same_or_better: &[("Current Rating (Max)", Direction::Higher), ("Rated Voltage (Max)", Direction::Higher)] }),
        ("Rotary Switches", CompatRule { primary: "Positions", must_match: &["Positions", "Number of Poles Per Deck"], same_or_better: &[("Current Rating", Direction::Higher), ("Voltage Rating (DC)", Direction::Higher)] }),
        ("SAW Resonators", CompatRule { primary: "Frequency", must_match: &["Frequency"], same_or_better: &[] }),
        ("Safety Capacitors", CompatRule { primary: "Capacitance", must_match: &["Ratings"], same_or_better: &[("Tolerance", Direction::Lower), ("Voltage(AC)", Direction::Higher)] }),
        ("Schottky Diodes", CompatRule { primary: "Voltage - DC Reverse(Vr)", must_match: &[], same_or_better: &[("Current - Rectified", Direction::Higher), ("Voltage - DC Reverse(Vr)", Direction::Higher), ("Voltage - Forward(Vf@If)", Direction::Lower)] }),
        ("Screw Terminal Blocks", CompatRule { primary: "Number of Positions or Pins", must_match: &["Number of Positions or Pins"], same_or_better: &[("Current Rating", Direction::Higher), ("Voltage Rating (Max)", Direction::Higher)] }),
        ("Semiconductor Discharge Tubes (TSS)", CompatRule { primary: "Peak off - state voltage(Vdrm)", must_match: &[], same_or_better: &[("Peak Pulse Current-Ipp (10/1000us)", Direction::Higher)] }),
        ("Shunts, Jumpers", CompatRule { primary: "Pitch", must_match: &["Pitch", "Number of Positions"], same_or_better: &[("Current Rating", Direction::Higher)] }),
        ("SiC Diodes", CompatRule { primary: "Voltage - DC Reverse(Vr)", must_match: &[], same_or_better: &[("Current - Rectified", Direction::Higher), ("Voltage - DC Reverse(Vr)", Direction::Higher)] }),
        ("Signal Relays", CompatRule { primary: "Coil Voltage", must_match: &["Coil Voltage", "Contact Form"], same_or_better: &[("Contact Rating", Direction::Higher), ("Switching Current(Max)", Direction::Higher)] }),
        ("Silicon Carbide Field Effect Transistor (MOSFET)", CompatRule { primary: "Drain to Source Voltage", must_match: &[], same_or_better: &[("Current - Continuous Drain(Id)", Direction::Higher), ("Drain to Source Voltage", Direction::Higher), ("RDS(on)", Direction::Lower)] }),
        ("Slide Switches", CompatRule { primary: "Circuit", must_match: &["Circuit", "Mounting Type"], same_or_better: &[("Current Rating", Direction::Higher), ("Voltage Rating", Direction::Higher)] }),
        ("Solid State Relays (MOS Output)", CompatRule { primary: "Load Voltage", must_match: &[], same_or_better: &[("Load Current", Direction::Higher), ("Load Voltage", Direction::Higher), ("RDS(on)", Direction::Lower)] }),
        ("Solid State Relays (Triac Output)", CompatRule { primary: "Load Voltage", must_match: &["Contact Form"], same_or_better: &[("Load Current", Direction::Higher), ("Load Voltage", Direction::Higher)] }),
        ("Speakers", CompatRule { primary: "Impedance", must_match: &["Impedance"], same_or_better: &[("Rated Power", Direction::Higher)] }),
        ("Super Barrier Rectifiers (SBR)", CompatRule { primary: "Voltage - DC Reverse(Vr)", must_match: &[], same_or_better: &[("Current - Rectified", Direction::Higher), ("Voltage - DC Reverse(Vr)", Direction::Higher), ("Voltage - Forward(Vf@If)", Direction::Lower)] }),
        ("Switching Diodes", CompatRule { primary: "Voltage - DC Reverse(Vr)", must_match: &[], same_or_better: &[("Current - Rectified", Direction::Higher), ("Voltage - DC Reverse(Vr)", Direction::Higher)] }),
        ("Tactile Switches", CompatRule { primary: "Mounting Type", must_match: &["Mounting Type"], same_or_better: &[("Contact Current", Direction::Higher), ("Voltage Rating", Direction::Higher)] }),
        ("Tantalum Capacitors", CompatRule { primary: "Capacitance", must_match: &[], same_or_better: &[("Tolerance", Direction::Lower), ("Voltage Rating", Direction::Higher)] }),
        ("Temperature Compensated Crystal Oscillators (TCXO)", CompatRule { primary: "Frequency", must_match: &["Frequency", "Output Type"], same_or_better: &[("Frequency Stability", Direction::Lower)] }),
        ("Thermal Fuses (TCO)", CompatRule { primary: "Rated Functioning Temperature", must_match: &["Rated Functioning Temperature"], same_or_better: &[("Current Rating", Direction::Higher), ("Voltage Rating", Direction::Higher)] }),
        ("Through Hole Ceramic Capacitors", CompatRule { primary: "Capacitance", must_match: &["Temperature Coefficient"], same_or_better: &[("Tolerance", Direction::Lower), ("Voltage Rating", Direction::Higher)] }),
        ("Through Hole Resistors", CompatRule { primary: "Resistance", must_match: &[], same_or_better: &[("Power(Watts)", Direction::Higher), ("Tolerance", Direction::Lower)] }),
        ("Toggle Switches", CompatRule { primary: "Circuit", must_match: &["Circuit"], same_or_better: &[("Current Rating", Direction::Higher), ("Voltage Rating (DC)", Direction::Higher)] }),
        ("Transistor, Photovoltaic Output Optoisolators", CompatRule { primary: "Isolation Voltage(Vrms)", must_match: &[], same_or_better: &[("Isolation Voltage(Vrms)", Direction::Higher)] }),
        ("Translators, Level Shifters", CompatRule { primary: "Channel Type", must_match: &["Channel Type"], same_or_better: &[] }),
        ("Triac, SCR Output Optoisolators", CompatRule { primary: "Load Voltage", must_match: &[], same_or_better: &[("Isolation Voltage(Vrms)", Direction::Higher), ("Load Current", Direction::Higher), ("Load Voltage", Direction::Higher)] }),
        ("USB Connectors", CompatRule { primary: "Connector Type", must_match: &["Connector Type", "Gender"], same_or_better: &[] }),
        ("Ultraviolet LEDs (UVLED)", CompatRule { primary: "Peak Wavelength", must_match: &["Peak Wavelength"], same_or_better: &[] }),
        ("Varistors", CompatRule { primary: "Varistor Voltage", must_match: &["Varistor Voltage"], same_or_better: &[("Clamping Voltage", Direction::Lower), ("Energy", Direction::Higher)] }),
        ("Vibration Motors", CompatRule { primary: "Voltage Rating", must_match: &[], same_or_better: &[("Current Rating", Direction::Higher), ("Voltage Rating", Direction::Higher)] }),
        ("Voltage Reference", CompatRule { primary: "Output Voltage", must_match: &["Output Voltage"], same_or_better: &[("Temperature Coefficient", Direction::Lower), ("Tolerance", Direction::Lower)] }),
        ("Voltage Regulators - Linear, Low Drop Out (LDO) Regulators", CompatRule { primary: "Output Voltage", must_match: &["Output Voltage"], same_or_better: &[("Output Current", Direction::Higher), ("Voltage Dropout", Direction::Lower)] }),
        ("Voltage-Controlled Crystal Oscillators (VCXOs)", CompatRule { primary: "Frequency", must_match: &["Frequency", "Output Type"], same_or_better: &[("Frequency Stability", Direction::Lower)] }),
        ("WiFi Modules", CompatRule { primary: "Voltage - Supply", must_match: &["Voltage - Supply"], same_or_better: &[("Output Power", Direction::Higher)] }),
        ("Wire To Board Connector", CompatRule { primary: "Pitch", must_match: &["Pitch", "Pins Structure"], same_or_better: &[("Current Rating", Direction::Higher), ("Voltage Rating", Direction::Higher)] }),
        ("Wireless Charging Coils", CompatRule { primary: "Inductance", must_match: &["Number of Coils"], same_or_better: &[("DC Resistance(DCR)", Direction::Lower)] }),
        ("XLR (Cannon) Connectors", CompatRule { primary: "Number of Pins", must_match: &["Number of Pins", "Gender"], same_or_better: &[("Current Rating", Direction::Higher), ("Voltage Rating", Direction::Higher)] }),
        ("Zener Diodes", CompatRule { primary: "Zener Voltage(Nom)", must_match: &["Zener Voltage(Nom)"], same_or_better: &[("Pd - Power Dissipation", Direction::Higher)] }),
    ])
}

pub fn dimension_spec_fields() -> HashSet<&'static str> {
    HashSet::from(["Diameter", "Height", "Height - Seated (Max)"])
}

pub fn string_match_specs() -> HashSet<&'static str> {
    HashSet::from([
        "B Constant (25℃/100℃)", "Channel Type", "Circuit", "Class", "Color",
        "Connector Type", "Contact Form", "Data Rate", "Data Rate(Max)", "Direction",
        "Driver Circuitry", "Encoder Type", "FET Type", "Gender", "Illumination Color",
        "Impedance", "Mounting Type", "Number of Capacitors", "Number of Cells",
        "Number of Coils", "Number of Forward Channels", "Number of H-bridges",
        "Number of Lines", "Number of Pins", "Number of Poles", "Number of Poles Per Deck",
        "Number of Positions", "Number of Positions or Pins", "Number of Resistors",
        "Number of Reverse Channels", "Number of Rows", "Number of Segments",
        "Number of Turns", "Output Type", "Peak Wavelength", "Pins Structure", "Pitch",
        "Positions", "Rated Functioning Temperature", "Ratings", "Self Lock / No Lock",
        "Temperature Coefficient", "Type", "Type of Battery", "type",
    ])
}

pub fn pin_count_specs() -> HashSet<&'static str> {
    HashSet::from([
        "Number of Pins", "Number of Positions", "Number of Positions or Pins",
        "Pin Structure", "Pins Structure",
    ])
}

fn normalize_pin_count(value: &str) -> String {
    let value = value.trim();
    let re_nxm = regex::Regex::new(r"^(\d+)\s*x\s*(\d+)\s*[Pp]?$").unwrap();
    if let Some(c) = re_nxm.captures(value) {
        let rows: i64 = c[1].parse().unwrap();
        let pins_per_row: i64 = c[2].parse().unwrap();
        return (rows * pins_per_row).to_string();
    }
    let re_1xn = regex::Regex::new(r"^1\s*x\s*(\d+)\s*[Pp]?$").unwrap();
    if let Some(c) = re_1xn.captures(value) {
        return c[1].to_string();
    }
    let re_np = regex::Regex::new(r"^(\d+)\s*[Pp]$").unwrap();
    if let Some(c) = re_np.captures(value) {
        return c[1].to_string();
    }
    let re_plain = regex::Regex::new(r"^(\d+)$").unwrap();
    if let Some(c) = re_plain.captures(value) {
        return c[1].to_string();
    }
    value.to_string()
}

/// Check if two spec values match (for must_match rules).
pub fn values_match(orig_val: &str, cand_val: &str, spec: &str) -> bool {
    if spec == "Impedance @ Frequency" {
        return impedance_at_freq_match(orig_val, cand_val);
    }
    if pin_count_specs().contains(spec) {
        return normalize_pin_count(orig_val) == normalize_pin_count(cand_val);
    }
    if string_match_specs().contains(spec) {
        return orig_val.trim().to_lowercase() == cand_val.trim().to_lowercase();
    }
    if let Some(SpecParser::Parser(parser)) = spec_parsers().get(spec) {
        let orig_parsed = parser(orig_val);
        let cand_parsed = parser(cand_val);
        return match (orig_parsed, cand_parsed) {
            (Some(o), Some(c)) => {
                if o == 0.0 {
                    c == 0.0
                } else {
                    (o - c).abs() / o.abs() < 0.02
                }
            }
            _ => true,
        };
    }
    orig_val.trim().to_lowercase() == cand_val.trim().to_lowercase()
}

/// Check if candidate spec meets same_or_better requirement.
pub fn spec_ok(orig_val: &str, cand_val: &str, spec: &str, direction: Direction) -> bool {
    let parser = match spec_parsers().get(spec) {
        Some(SpecParser::Parser(p)) => *p,
        _ => return true,
    };
    let (orig_parsed, cand_parsed) = match (parser(orig_val), parser(cand_val)) {
        (Some(o), Some(c)) => (o, c),
        _ => return true,
    };
    match direction {
        Direction::Higher => cand_parsed >= orig_parsed * 0.98,
        Direction::Lower => cand_parsed <= orig_parsed * 1.02,
    }
}

#[derive(Debug, Clone, Default)]
pub struct VerifyInfo {
    pub specs_verified: Vec<String>,
    pub specs_unparseable: Vec<String>,
}

fn get_spec<'a>(specs: &'a serde_json::Value, name: &str) -> Option<&'a str> {
    specs.get(name).and_then(|v| v.as_str())
}

/// Check if candidate is a compatible alternative for original.
/// `original`/`candidate` are `{"specs": {...}}`-shaped values, matching the
/// component dict shape the search/db layer produces.
pub fn is_compatible_alternative(original: &serde_json::Value, candidate: &serde_json::Value, subcategory: &str) -> (bool, VerifyInfo) {
    let rules = match compatibility_rules().remove(subcategory) {
        Some(r) => r,
        None => return (true, VerifyInfo::default()),
    };

    let empty = serde_json::json!({});
    let orig_specs = original.get("specs").unwrap_or(&empty);
    let cand_specs = candidate.get("specs").unwrap_or(&empty);

    let mut info = VerifyInfo::default();

    for spec in rules.must_match {
        let orig_val = get_spec(orig_specs, spec);
        let cand_val = get_spec(cand_specs, spec);
        match (orig_val, cand_val) {
            (Some(o), Some(c)) if !o.is_empty() && !c.is_empty() => {
                if !values_match(o, c, spec) {
                    return (false, info);
                }
                info.specs_verified.push(spec.to_string());
            }
            (Some(_), Some(_)) => info.specs_unparseable.push(spec.to_string()),
            (Some(_), None) | (None, Some(_)) => info.specs_unparseable.push(spec.to_string()),
            (None, None) => {}
        }
    }

    for (spec, direction) in rules.same_or_better {
        let orig_val = get_spec(orig_specs, spec);
        let cand_val = get_spec(cand_specs, spec);
        match (orig_val, cand_val) {
            (Some(o), Some(c)) if !o.is_empty() && !c.is_empty() => {
                if let Some(SpecParser::Parser(parser)) = spec_parsers().get(*spec) {
                    if parser(o).is_some() && parser(c).is_some() {
                        if !spec_ok(o, c, spec, *direction) {
                            return (false, info);
                        }
                        info.specs_verified.push(spec.to_string());
                    } else {
                        info.specs_unparseable.push(spec.to_string());
                    }
                } else {
                    info.specs_unparseable.push(spec.to_string());
                }
            }
            (Some(_), Some(_)) => info.specs_unparseable.push(spec.to_string()),
            (Some(_), None) | (None, Some(_)) => info.specs_unparseable.push(spec.to_string()),
            (None, None) => {}
        }
    }

    (true, info)
}

/// Verify candidate has same primary spec value as original.
pub fn verify_primary_spec_match(original: &serde_json::Value, candidate: &serde_json::Value, primary_attr: &str) -> bool {
    let empty = serde_json::json!({});
    let orig_value = get_spec(original.get("specs").unwrap_or(&empty), primary_attr);
    let cand_value = get_spec(candidate.get("specs").unwrap_or(&empty), primary_attr);
    match (orig_value, cand_value) {
        (Some(o), Some(c)) if !o.is_empty() && !c.is_empty() => values_match(o, c, primary_attr),
        _ => true,
    }
}

/// Score an alternative part for ranking. Returns (total_score, breakdown).
pub fn score_alternative(part: &serde_json::Value, original: &serde_json::Value, min_price_in_results: Option<f64>) -> (i64, HashMap<String, i64>) {
    let mut score: i64 = 0;
    let mut breakdown: HashMap<String, i64> = HashMap::new();

    let lib_type = part.get("library_type").and_then(|v| v.as_str());
    if matches!(lib_type, Some("basic") | Some("preferred")) {
        score += 1000;
        breakdown.insert("library_type".to_string(), 1000);
    } else {
        breakdown.insert("library_type".to_string(), 0);
    }

    let stock = part.get("stock").and_then(|v| v.as_i64()).unwrap_or(0);
    let avail_score = if stock >= 10000 {
        70
    } else if stock >= 1000 {
        50
    } else if stock >= 100 {
        30
    } else {
        -10
    };
    score += avail_score;
    breakdown.insert("availability".to_string(), avail_score);

    if part.get("has_easyeda_footprint").and_then(|v| v.as_bool()).unwrap_or(false) {
        score += 20;
        breakdown.insert("easyeda".to_string(), 20);
    } else {
        breakdown.insert("easyeda".to_string(), 0);
    }

    let same_mfr = part.get("manufacturer").and_then(|v| v.as_str())
        == original.get("manufacturer").and_then(|v| v.as_str());
    if same_mfr {
        score += 10;
        breakdown.insert("same_manufacturer".to_string(), 10);
    } else {
        breakdown.insert("same_manufacturer".to_string(), 0);
    }

    let part_price = part.get("price").and_then(|v| v.as_f64());
    let price_score = match (part_price, min_price_in_results) {
        (Some(pp), Some(mp)) if pp > 0.0 && mp > 0.0 => {
            let price_ratio = mp / pp;
            (10.0 * price_ratio).floor().min(10.0) as i64
        }
        _ => 0,
    };
    score += price_score;
    breakdown.insert("price".to_string(), price_score);

    (score, breakdown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- TestValuesMatch ---
    #[test]
    fn test_resistance_match() {
        assert!(values_match("10kΩ", "10kΩ", "Resistance"));
        assert!(values_match("10kΩ", "10K", "Resistance"));
        assert!(!values_match("10kΩ", "20kΩ", "Resistance"));
    }
    #[test]
    fn test_string_match_spec() {
        assert!(values_match("X7R", "X7R", "Temperature Coefficient"));
        assert!(values_match("x7r", "X7R", "Temperature Coefficient"));
        assert!(!values_match("X7R", "X5R", "Temperature Coefficient"));
    }
    #[test]
    fn test_color_match() {
        assert!(values_match("Red", "red", "Illumination Color"));
        assert!(!values_match("Red", "Blue", "Illumination Color"));
    }
    #[test]
    fn test_voltage_match_with_tolerance() {
        assert!(values_match("25V", "25V", "Voltage Rating"));
        assert!(values_match("25V", "25.4V", "Voltage Rating"));
        assert!(!values_match("25V", "30V", "Voltage Rating"));
    }

    // --- TestSpecOk ---
    #[test]
    fn test_higher_is_better() {
        assert!(spec_ok("25V", "50V", "Voltage Rating", Direction::Higher));
        assert!(!spec_ok("50V", "25V", "Voltage Rating", Direction::Higher));
    }
    #[test]
    fn test_lower_is_better() {
        assert!(spec_ok("5%", "1%", "Tolerance", Direction::Lower));
        assert!(!spec_ok("1%", "5%", "Tolerance", Direction::Lower));
    }
    #[test]
    fn test_tolerance_margin() {
        assert!(spec_ok("10V", "9.9V", "Voltage Rating", Direction::Higher));
    }

    // --- TestIsCompatibleAlternative ---
    #[test]
    fn test_resistor_compatible() {
        let original = json!({"specs": {"Resistance": "10kΩ", "Tolerance": "5%", "Power(Watts)": "1/4W"}});
        let candidate = json!({"specs": {"Resistance": "10kΩ", "Tolerance": "1%", "Power(Watts)": "1/2W"}});
        let (is_compat, info) = is_compatible_alternative(&original, &candidate, "Chip Resistor - Surface Mount");
        assert!(is_compat);
        assert!(info.specs_verified.contains(&"Tolerance".to_string()));
        assert!(info.specs_verified.contains(&"Power(Watts)".to_string()));
    }
    #[test]
    fn test_resistor_incompatible_tolerance() {
        let original = json!({"specs": {"Resistance": "10kΩ", "Tolerance": "1%", "Power(Watts)": "1/4W"}});
        let candidate = json!({"specs": {"Resistance": "10kΩ", "Tolerance": "5%", "Power(Watts)": "1/4W"}});
        let (is_compat, _) = is_compatible_alternative(&original, &candidate, "Chip Resistor - Surface Mount");
        assert!(!is_compat);
    }
    #[test]
    fn test_capacitor_must_match_dielectric() {
        let original = json!({"specs": {"Capacitance": "100nF", "Voltage Rating": "25V", "Temperature Coefficient": "X7R"}});
        let candidate = json!({"specs": {"Capacitance": "100nF", "Voltage Rating": "50V", "Temperature Coefficient": "X5R"}});
        let (is_compat, _) = is_compatible_alternative(&original, &candidate, "Multilayer Ceramic Capacitors MLCC - SMD/SMT");
        assert!(!is_compat);
    }
    #[test]
    fn test_capacitor_compatible_higher_voltage() {
        let original = json!({"specs": {"Capacitance": "100nF", "Voltage Rating": "25V", "Temperature Coefficient": "X7R", "Tolerance": "10%"}});
        let candidate = json!({"specs": {"Capacitance": "100nF", "Voltage Rating": "50V", "Temperature Coefficient": "X7R", "Tolerance": "5%"}});
        let (is_compat, _) = is_compatible_alternative(&original, &candidate, "Multilayer Ceramic Capacitors MLCC - SMD/SMT");
        assert!(is_compat);
    }
    #[test]
    fn test_led_must_match_color() {
        let original = json!({"specs": {"Illumination Color": "Red"}});
        let candidate = json!({"specs": {"Illumination Color": "Blue"}});
        let (is_compat, _) = is_compatible_alternative(&original, &candidate, "LED Indication - Discrete");
        assert!(!is_compat);
    }
    #[test]
    fn test_unsupported_category_passes() {
        let original = json!({"specs": {"Some Spec": "value"}});
        let candidate = json!({"specs": {"Some Spec": "different"}});
        let (is_compat, info) = is_compatible_alternative(&original, &candidate, "Unknown Category That Does Not Exist");
        assert!(is_compat);
        assert!(info.specs_verified.is_empty());
    }

    // --- TestVerifyPrimarySpecMatch ---
    #[test]
    fn test_verify_resistance_match() {
        let original = json!({"specs": {"Resistance": "10kΩ"}});
        let candidate = json!({"specs": {"Resistance": "10kΩ"}});
        assert!(verify_primary_spec_match(&original, &candidate, "Resistance"));
    }
    #[test]
    fn test_verify_resistance_mismatch() {
        let original = json!({"specs": {"Resistance": "10kΩ"}});
        let candidate = json!({"specs": {"Resistance": "20kΩ"}});
        assert!(!verify_primary_spec_match(&original, &candidate, "Resistance"));
    }
    #[test]
    fn test_verify_missing_spec_passes() {
        let original = json!({"specs": {"Resistance": "10kΩ"}});
        let candidate = json!({"specs": {}});
        assert!(verify_primary_spec_match(&original, &candidate, "Resistance"));
    }

    // --- TestCompatibilityRulesCoverage ---
    #[test]
    fn test_resistors_covered() {
        let rules = compatibility_rules();
        assert!(rules.contains_key("Chip Resistor - Surface Mount"));
        assert!(rules.contains_key("Through Hole Resistors"));
    }
    #[test]
    fn test_capacitors_covered() {
        let rules = compatibility_rules();
        assert!(rules.contains_key("Multilayer Ceramic Capacitors MLCC - SMD/SMT"));
        assert!(rules.contains_key("Aluminum Electrolytic Capacitors - SMD"));
        assert!(rules.contains_key("Tantalum Capacitors"));
    }
    #[test]
    fn test_inductors_covered() {
        let rules = compatibility_rules();
        assert!(rules.contains_key("Inductors (SMD)"));
        assert!(rules.contains_key("Power Inductors"));
        assert!(rules.contains_key("Ferrite Beads"));
    }
    #[test]
    fn test_semiconductors_covered() {
        let rules = compatibility_rules();
        assert!(rules.contains_key("MOSFETs"));
        assert!(rules.contains_key("Bipolar (BJT)"));
        assert!(rules.contains_key("Schottky Diodes"));
        assert!(rules.contains_key("Zener Diodes"));
    }
    #[test]
    fn test_leds_covered() {
        let rules = compatibility_rules();
        assert!(rules.contains_key("LED Indication - Discrete"));
        assert!(rules.contains_key("LED - High Brightness"));
    }
    #[test]
    fn test_timing_covered() {
        let rules = compatibility_rules();
        assert!(rules.contains_key("Crystals"));
        assert!(rules.contains_key("Crystal Oscillators"));
    }
    #[test]
    fn test_switches_covered() {
        let rules = compatibility_rules();
        assert!(rules.contains_key("Tactile Switches"));
        assert!(rules.contains_key("Toggle Switches"));
    }
    #[test]
    fn test_connectors_covered() {
        let rules = compatibility_rules();
        assert!(rules.contains_key("Pin Headers"));
        assert!(rules.contains_key("USB Connectors"));
    }

    // --- TestScoreAlternative ---
    #[test]
    fn test_basic_library_gets_high_score() {
        let part = json!({"library_type": "basic", "stock": 10000, "price": 0.01});
        let original = json!({"manufacturer": "Other"});
        let (score, breakdown) = score_alternative(&part, &original, Some(0.01));
        assert_eq!(breakdown["library_type"], 1000);
        assert!(score >= 1000);
    }
    #[test]
    fn test_extended_library_low_score() {
        let part = json!({"library_type": "extended", "stock": 10000, "price": 0.01});
        let original = json!({"manufacturer": "Other"});
        let (_, breakdown) = score_alternative(&part, &original, Some(0.01));
        assert_eq!(breakdown["library_type"], 0);
    }
    #[test]
    fn test_high_stock_bonus() {
        let part = json!({"library_type": "extended", "stock": 50000, "price": 0.01});
        let original = json!({"manufacturer": "Other"});
        let (_, breakdown) = score_alternative(&part, &original, Some(0.01));
        assert_eq!(breakdown["availability"], 70);
    }
    #[test]
    fn test_same_manufacturer_bonus() {
        let part = json!({"library_type": "extended", "stock": 1000, "price": 0.01, "manufacturer": "Samsung"});
        let original = json!({"manufacturer": "Samsung"});
        let (_, breakdown) = score_alternative(&part, &original, Some(0.01));
        assert_eq!(breakdown["same_manufacturer"], 10);
    }

    // --- TestScoreAlternativePrice ---
    #[test]
    fn test_price_missing_field() {
        let part = json!({"library_type": "extended", "stock": 1000});
        let original = json!({});
        let (_, breakdown) = score_alternative(&part, &original, Some(0.01));
        assert_eq!(breakdown["price"], 0);
    }
    #[test]
    fn test_price_min_price_none() {
        let part = json!({"library_type": "extended", "stock": 1000, "price": 0.01});
        let original = json!({});
        let (_, breakdown) = score_alternative(&part, &original, None);
        assert_eq!(breakdown["price"], 0);
    }
    #[test]
    fn test_price_fractional_ratio() {
        // price_ratio = 0.01 / 0.03 = 0.333..., 10 * 0.333 = 3.33, floor = 3
        let part = json!({"library_type": "extended", "stock": 1000, "price": 0.03});
        let original = json!({});
        let (_, breakdown) = score_alternative(&part, &original, Some(0.01));
        assert_eq!(breakdown["price"], 3);
    }
}
