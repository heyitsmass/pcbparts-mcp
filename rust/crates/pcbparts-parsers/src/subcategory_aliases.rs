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
