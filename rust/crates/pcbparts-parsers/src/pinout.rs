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
