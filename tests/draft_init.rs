mod common;

use std::fs;

use common::{
    assert_json_envelope_shape, fixture_path, parse_stdout_json, profile_cmd, temp_workspace,
};
use serde_yaml::Value as YamlValue;

#[test]
fn draft_init_uses_header_order_for_include_columns() {
    let workspace = temp_workspace();
    let out_path = workspace.path().join("init.yaml");

    let assert = profile_cmd()
        .arg("draft")
        .arg("init")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .arg("--out")
        .arg(&out_path)
        .assert();
    common::assert_success_exit!(assert);

    let yaml = load_yaml(&out_path);
    let include_columns = yaml["include_columns"]
        .as_sequence()
        .expect("include_columns should be a sequence")
        .iter()
        .map(|item| item.as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();

    assert_eq!(
        include_columns,
        vec!["loan_id", "balance", "rate", "property_type"]
    );
}

#[test]
fn draft_init_respects_explicit_key() {
    let workspace = temp_workspace();
    let out_path = workspace.path().join("explicit_key.yaml");

    let assert = profile_cmd()
        .arg("draft")
        .arg("init")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .arg("--out")
        .arg(&out_path)
        .arg("--key")
        .arg("balance")
        .assert();
    common::assert_success_exit!(assert);

    let yaml = load_yaml(&out_path);
    let keys = yaml["key"]
        .as_sequence()
        .expect("key should be a sequence")
        .iter()
        .map(|item| item.as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    assert_eq!(keys, vec!["balance"]);
}

#[test]
fn draft_init_auto_key_selects_viable_candidate() {
    let workspace = temp_workspace();
    let out_path = workspace.path().join("auto_key.yaml");

    let assert = profile_cmd()
        .arg("draft")
        .arg("init")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .arg("--out")
        .arg(&out_path)
        .arg("--key")
        .arg("auto")
        .assert();
    common::assert_success_exit!(assert);

    let yaml = load_yaml(&out_path);
    let keys = yaml["key"]
        .as_sequence()
        .expect("key should be a sequence")
        .iter()
        .map(|item| item.as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    assert_eq!(keys, vec!["loan_id"]);
}

#[test]
fn draft_init_auto_key_falls_back_to_empty_when_no_viable_candidate() {
    let workspace = temp_workspace();
    let out_path = workspace.path().join("auto_fallback.yaml");

    let assert = profile_cmd()
        .arg("draft")
        .arg("init")
        .arg(fixture_path("datasets/valid/no_unique_key.csv"))
        .arg("--out")
        .arg(&out_path)
        .arg("--key")
        .arg("auto")
        .assert();
    common::assert_success_exit!(assert);

    let yaml = load_yaml(&out_path);
    let keys = yaml["key"]
        .as_sequence()
        .expect("key should be a sequence")
        .iter()
        .map(|item| item.as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    assert!(keys.is_empty(), "expected key to be empty fallback");
}

#[test]
fn draft_init_auto_key_refuses_when_dataset_has_no_data_rows() {
    let workspace = temp_workspace();
    let out_path = workspace.path().join("auto_empty.yaml");

    let assert = profile_cmd()
        .arg("--json")
        .arg("draft")
        .arg("init")
        .arg(fixture_path("datasets/invalid/header_only.csv"))
        .arg("--out")
        .arg(&out_path)
        .arg("--key")
        .arg("auto")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope.get("outcome").and_then(|v| v.as_str()),
        Some("REFUSAL")
    );
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("code"))
            .and_then(|v| v.as_str()),
        Some("E_EMPTY")
    );
}

#[test]
fn draft_init_json_wraps_result_envelope() {
    let workspace = temp_workspace();
    let out_path = workspace.path().join("init_json.yaml");

    let assert = profile_cmd()
        .arg("--json")
        .arg("draft")
        .arg("init")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .arg("--out")
        .arg(&out_path)
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope.get("subcommand").and_then(|v| v.as_str()),
        Some("draft init")
    );
    assert_eq!(
        envelope.get("outcome").and_then(|v| v.as_str()),
        Some("SUCCESS")
    );
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("path"))
            .and_then(|v| v.as_str()),
        Some(out_path.to_string_lossy().as_ref())
    );
}

fn load_yaml(path: &std::path::Path) -> YamlValue {
    let content = fs::read_to_string(path).expect("generated YAML should be readable");
    serde_yaml::from_str(&content).expect("generated YAML should parse")
}
