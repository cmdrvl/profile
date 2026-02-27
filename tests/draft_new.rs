mod common;

use std::fs;

use common::{assert_json_envelope_shape, parse_stdout_json, profile_cmd, temp_workspace};

#[test]
fn draft_new_writes_deterministic_template() {
    let workspace = temp_workspace();
    let out_path = workspace.path().join("draft.yaml");

    let assert = profile_cmd()
        .arg("draft")
        .arg("new")
        .arg("--format")
        .arg("csv")
        .arg("--out")
        .arg(&out_path)
        .assert();
    common::assert_success_exit!(assert);

    let content = fs::read_to_string(&out_path).expect("draft output should be readable");
    let expected = "\
schema_version: 1
status: draft
format: csv
equivalence:
  float_decimals: 6
  trim_strings: true
key: []
include_columns: []
";
    assert_eq!(content, expected);
}

#[test]
fn draft_new_output_validates_successfully() {
    let workspace = temp_workspace();
    let out_path = workspace.path().join("draft.yaml");

    let create_assert = profile_cmd()
        .arg("draft")
        .arg("new")
        .arg("--format")
        .arg("csv")
        .arg("--out")
        .arg(&out_path)
        .assert();
    common::assert_success_exit!(create_assert);

    let validate_assert = profile_cmd()
        .arg("--no-witness")
        .arg("validate")
        .arg(&out_path)
        .assert();
    common::assert_success_exit!(validate_assert);
}

#[test]
fn draft_new_json_wraps_result_envelope() {
    let workspace = temp_workspace();
    let out_path = workspace.path().join("draft_json.yaml");

    let assert = profile_cmd()
        .arg("--json")
        .arg("draft")
        .arg("new")
        .arg("--format")
        .arg("csv")
        .arg("--out")
        .arg(&out_path)
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope.get("subcommand").and_then(|v| v.as_str()),
        Some("draft new")
    );
    assert_eq!(
        envelope.get("outcome").and_then(|v| v.as_str()),
        Some("SUCCESS")
    );
    assert_eq!(envelope.get("exit_code").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("path"))
            .and_then(|v| v.as_str()),
        Some(out_path.to_string_lossy().as_ref())
    );
}

#[test]
fn draft_new_rejects_unsupported_format() {
    let workspace = temp_workspace();
    let out_path = workspace.path().join("bad.yaml");

    let assert = profile_cmd()
        .arg("draft")
        .arg("new")
        .arg("--format")
        .arg("json")
        .arg("--out")
        .arg(&out_path)
        .assert();
    common::assert_refusal_exit!(assert);
}
