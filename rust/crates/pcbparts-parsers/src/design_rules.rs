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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::Path;
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
