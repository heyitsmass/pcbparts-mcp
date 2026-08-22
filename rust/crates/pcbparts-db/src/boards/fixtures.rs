//! Shared test fixture: builds the exact boards.db produced by
//! scripts/build_boards_db.py from the Python test suite's SAMPLE_BOARDS,
//! dumped once via a throwaway script so these rows are byte-identical to
//! what the real builder emits.
use rusqlite::Connection;
use std::collections::HashMap;

use super::SCHEMA;

pub(crate) fn test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(SCHEMA).unwrap();

    struct BoardRow {
        slug: &'static str, name: &'static str, org: &'static str, org_display: &'static str,
        source: &'static str, format: &'static str, description: &'static str,
        key_coverage: &'static str, layers: i64, width_mm: Option<f64>, height_mm: Option<f64>,
        min_trace: Option<&'static str>, min_clearance: Option<&'static str>,
        component_count: i64, ic_count: i64, net_count: i64,
        key_ics_text: &'static str, all_ics_text: &'static str,
        nets_json: Option<&'static str>, positions_json: Option<&'static str>,
        copper_pours_json: Option<&'static str>, neighborhoods_json: Option<&'static str>,
        tags: &'static [&'static str], key_ics: &'static [&'static str],
    }

    let boards = vec![
        BoardRow {
            slug: "adafruit-motor-shield", name: "Adafruit Motor Shield", org: "adafruit", org_display: "Adafruit",
            source: "adafruit/Motor-Shield", format: "eagle", description: "A motor driver shield for Arduino with DRV8825",
            key_coverage: "DRV8825 stepper motor driver", layers: 2, width_mm: Some(68.6), height_mm: Some(53.3),
            min_trace: None, min_clearance: None, component_count: 4, ic_count: 1, net_count: 2,
            key_ics_text: "DRV8825", all_ics_text: "DRV8825",
            nets_json: Some(r#"[{"name": "STEP", "pins": ["U1.STEP", "R1.1"]}, {"name": "GND", "pins": ["U1.GND", "C1.2", "C2.2", "R1.2"]}]"#),
            positions_json: None, copper_pours_json: None,
            neighborhoods_json: Some(r#"[{"ref": "U1", "value": "DRV8825", "pins": {"STEP": [{"ref": "R1", "value": "10kohm", "role": "pulldown"}], "_decoupling": [{"ref": "C1", "value": "100nF", "role": "decoupling"}, {"ref": "C2", "value": "47uF", "role": "decoupling"}]}}]"#),
            tags: &["motor-control"], key_ics: &["DRV8825"],
        },
        BoardRow {
            slug: "ble-sensor-node", name: "BLE Sensor Node", org: "SolderedElectronics", org_display: "Soldered Electronics",
            source: "SolderedElectronics/BLE-Sensor", format: "kicad7", description: "A Bluetooth Low Energy sensor node with BME280",
            key_coverage: "nRF52840 BLE with BME280 sensor", layers: 4, width_mm: Some(30.0), height_mm: Some(20.0),
            min_trace: None, min_clearance: None, component_count: 4, ic_count: 2, net_count: 4,
            key_ics_text: "nRF52840 BME280", all_ics_text: "nRF52840 BME280",
            nets_json: Some(r#"[{"name": "SDA", "pins": ["U1.SDA", "U2.SDA"]}, {"name": "SCL", "pins": ["U1.SCL", "U2.SCL"]}, {"name": "3V3", "pins": ["U1.VCC", "C1.1", "U2.VCC", "C2.1"]}, {"name": "GND", "pins": ["U1.GND", "C1.2", "U2.GND", "C2.2"]}]"#),
            positions_json: None, copper_pours_json: None,
            neighborhoods_json: Some(r#"[{"ref": "U1", "value": "nRF52840", "pins": {"SDA": [{"ref": "U2", "value": "BME280", "role": "ic"}], "SCL": [{"ref": "U2", "value": "BME280", "role": "ic"}], "_decoupling": [{"ref": "C1", "value": "100nF", "role": "decoupling"}]}}, {"ref": "U2", "value": "BME280", "pins": {"SDA": [{"ref": "U1", "value": "nRF52840", "role": "ic"}], "SCL": [{"ref": "U1", "value": "nRF52840", "role": "ic"}], "_decoupling": [{"ref": "C2", "value": "100nF", "role": "decoupling"}]}}]"#),
            tags: &["bluetooth", "sensors"], key_ics: &["BME280", "nRF52840"],
        },
        BoardRow {
            slug: "minimal-led-driver", name: "Minimal LED Driver", org: "maker", org_display: "Maker",
            source: "maker/led-driver", format: "eagle", description: "A simple constant-current LED driver with TPS61169",
            key_coverage: "TPS61169 constant-current LED driver", layers: 2, width_mm: None, height_mm: None,
            min_trace: None, min_clearance: None, component_count: 3, ic_count: 1, net_count: 0,
            key_ics_text: "TPS61169", all_ics_text: "TPS61169",
            nets_json: None, positions_json: None, copper_pours_json: None, neighborhoods_json: None,
            tags: &["led-driver"], key_ics: &["TPS61169"],
        },
        BoardRow {
            slug: "sparkfun-mcp73831-charger", name: "SparkFun MCP73831 Charger", org: "sparkfun", org_display: "SparkFun",
            source: "sparkfun/MCP73831-Charger", format: "eagle", description: "A simple LiPo charger breakout with MCP73831",
            key_coverage: "MCP73831 LiPo charging circuit", layers: 2, width_mm: None, height_mm: None,
            min_trace: None, min_clearance: None, component_count: 4, ic_count: 1, net_count: 4,
            key_ics_text: "MCP73831", all_ics_text: "MCP73831",
            nets_json: Some(r#"[{"name": "PROG", "pins": ["U1.PROG", "R1.1"]}, {"name": "STAT", "pins": ["U1.STAT", "D1.1"]}, {"name": "VCC", "pins": ["U1.VCC", "C1.1"]}, {"name": "GND", "pins": ["U1.GND", "C1.2", "R1.2", "D1.2"]}]"#),
            positions_json: None, copper_pours_json: None,
            neighborhoods_json: Some(r#"[{"ref": "U1", "value": "MCP73831", "pins": {"PROG": [{"ref": "R1", "value": "2kohm", "role": "resistor"}], "STAT": [{"ref": "D1", "value": "red LED", "role": "diode"}], "_decoupling": [{"ref": "C1", "value": "4.7uF", "role": "decoupling"}]}}]"#),
            tags: &["battery-charging", "power-supply"], key_ics: &["MCP73831"],
        },
        BoardRow {
            slug: "test-esp32-board", name: "Test ESP32 Board", org: "testorg", org_display: "Testorg",
            source: "testorg/test-esp32", format: "kicad7", description: "An ESP32 devkit with WiFi and battery charging",
            key_coverage: "ESP32-S3 WiFi devkit with MCP73831 battery charging", layers: 4, width_mm: Some(50.0), height_mm: Some(25.0),
            min_trace: Some("0.15mm"), min_clearance: Some("0.15mm"), component_count: 8, ic_count: 2, net_count: 5,
            key_ics_text: "ESP32-S3 MCP73831", all_ics_text: "ESP32-S3 MCP73831",
            nets_json: Some(r#"[{"name": "SDA", "pins": ["U1.SDA", "R3.1"]}, {"name": "SCL", "pins": ["U1.SCL", "R1.1"]}, {"name": "PROG", "pins": ["U2.PROG", "R2.1"]}, {"name": "3V3", "pins": ["U1.VCC", "C1.1", "U2.VCC", "C2.1"]}, {"name": "GND", "pins": ["U1.GND", "C1.2", "U2.GND", "C2.2"]}]"#),
            positions_json: Some(r#"[{"ref": "U1", "x": 10.0, "y": 10.0}, {"ref": "U2", "x": 20.0, "y": 10.0}]"#),
            copper_pours_json: Some(r#"[{"layer": "B.Cu", "net": "GND"}]"#),
            neighborhoods_json: Some(r#"[{"ref": "U1", "value": "ESP32-S3", "pins": {"SDA": [{"ref": "R3", "value": "100kohm", "role": "pullup"}], "SCL": [{"ref": "R1", "value": "10kohm", "role": "resistor"}], "_decoupling": [{"ref": "C1", "value": "100nF", "role": "decoupling"}]}}, {"ref": "U2", "value": "MCP73831", "pins": {"PROG": [{"ref": "R2", "value": "4.7kohm", "role": "resistor"}], "_decoupling": [{"ref": "C2", "value": "10uF", "role": "decoupling"}]}}]"#),
            tags: &["battery-charging", "sensors"], key_ics: &["ESP32-S3", "MCP73831"],
        },
    ];

    struct CompRow { board_slug: &'static str, ref_: &'static str, value: &'static str, footprint: &'static str, decouples: Option<&'static str>, pullup: Option<&'static str>, pulldown: Option<&'static str> }
    let comps = vec![
        CompRow { board_slug: "adafruit-motor-shield", ref_: "U1", value: "DRV8825", footprint: "HTSSOP-28", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "adafruit-motor-shield", ref_: "C1", value: "100nF", footprint: "0402", decouples: Some("U1"), pullup: None, pulldown: None },
        CompRow { board_slug: "adafruit-motor-shield", ref_: "C2", value: "47uF", footprint: "1206", decouples: Some("U1"), pullup: None, pulldown: None },
        CompRow { board_slug: "adafruit-motor-shield", ref_: "R1", value: "10kohm", footprint: "0402", decouples: None, pullup: None, pulldown: Some("STEP") },
        CompRow { board_slug: "ble-sensor-node", ref_: "U1", value: "nRF52840", footprint: "QFN-48", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "ble-sensor-node", ref_: "U2", value: "BME280", footprint: "LGA-8", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "ble-sensor-node", ref_: "C1", value: "100nF", footprint: "0402", decouples: Some("U1"), pullup: None, pulldown: None },
        CompRow { board_slug: "ble-sensor-node", ref_: "C2", value: "100nF", footprint: "0402", decouples: Some("U2"), pullup: None, pulldown: None },
        CompRow { board_slug: "minimal-led-driver", ref_: "U1", value: "TPS61169", footprint: "SOT-23-5", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "minimal-led-driver", ref_: "L1", value: "10uH", footprint: "1210", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "minimal-led-driver", ref_: "R1", value: "1ohm", footprint: "0402", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "sparkfun-mcp73831-charger", ref_: "U1", value: "MCP73831", footprint: "SOT-23-5", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "sparkfun-mcp73831-charger", ref_: "C1", value: "4.7uF", footprint: "0402", decouples: Some("U1"), pullup: None, pulldown: None },
        CompRow { board_slug: "sparkfun-mcp73831-charger", ref_: "R1", value: "2kohm", footprint: "0402", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "sparkfun-mcp73831-charger", ref_: "D1", value: "red LED", footprint: "0603", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "test-esp32-board", ref_: "U1", value: "ESP32-S3", footprint: "QFN-48", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "test-esp32-board", ref_: "U2", value: "MCP73831", footprint: "SOT-23-5", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "test-esp32-board", ref_: "R1", value: "10kohm", footprint: "0402", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "test-esp32-board", ref_: "R2", value: "4.7kohm", footprint: "0402", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "test-esp32-board", ref_: "C1", value: "100nF", footprint: "0402", decouples: Some("U1"), pullup: None, pulldown: None },
        CompRow { board_slug: "test-esp32-board", ref_: "C2", value: "10uF", footprint: "0805", decouples: Some("U2"), pullup: None, pulldown: None },
        CompRow { board_slug: "test-esp32-board", ref_: "C3", value: "4.7uF", footprint: "0402", decouples: None, pullup: None, pulldown: None },
        CompRow { board_slug: "test-esp32-board", ref_: "R3", value: "100kohm", footprint: "0402", decouples: None, pullup: Some("SDA"), pulldown: None },
    ];

    let mut board_ids: HashMap<&str, i64> = HashMap::new();
    for b in &boards {
        conn.execute(
            "INSERT INTO boards (slug, name, org, org_display, source, format, description, key_coverage, layers, width_mm, height_mm, min_trace, min_clearance, min_drill, min_via, component_count, ic_count, net_count, key_ics_text, all_ics_text, nets_json, positions_json, copper_pours_json, neighborhoods_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,NULL,NULL,?14,?15,?16,?17,?18,?19,?20,?21,?22)",
            rusqlite::params![b.slug, b.name, b.org, b.org_display, b.source, b.format, b.description, b.key_coverage, b.layers, b.width_mm, b.height_mm, b.min_trace, b.min_clearance, b.component_count, b.ic_count, b.net_count, b.key_ics_text, b.all_ics_text, b.nets_json, b.positions_json, b.copper_pours_json, b.neighborhoods_json],
        ).unwrap();
        let id = conn.last_insert_rowid();
        board_ids.insert(b.slug, id);
        for t in b.tags {
            conn.execute("INSERT INTO board_tags VALUES (?1, ?2)", rusqlite::params![id, t]).unwrap();
        }
        for ic in b.key_ics {
            conn.execute("INSERT INTO board_key_ics VALUES (?1, ?2)", rusqlite::params![id, ic]).unwrap();
        }
    }
    for c in &comps {
        let board_id = board_ids[c.board_slug];
        conn.execute(
            "INSERT INTO board_components (board_id, ref, value, footprint, decouples, pullup, pulldown) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![board_id, c.ref_, c.value, c.footprint, c.decouples, c.pullup, c.pulldown],
        ).unwrap();
    }

    // Populate FTS exactly as build_boards_db.py does: per-board tags_text joined from board_tags.
    for b in &boards {
        let tags_text = b.tags.join(" ");
        conn.execute(
            "INSERT INTO boards_fts (slug, name, description, key_coverage, tags_text, key_ics_text, all_ics_text, org_display) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            rusqlite::params![b.slug, b.name, b.description, b.key_coverage, tags_text, b.key_ics_text, b.all_ics_text, b.org_display],
        ).unwrap();
    }

    conn
}
