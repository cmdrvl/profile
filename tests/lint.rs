mod common;

use std::fs;

use common::{
    assert_json_envelope_shape, fixture_path, parse_stdout_json, profile_cmd, temp_workspace,
};
use predicates::prelude::predicate;

#[test]
fn lint_human_output_contract_for_clean_profile() {
    profile_cmd()
        .arg("lint")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .arg("--against")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .assert()
        .code(common::EXIT_SUCCESS)
        .stdout(predicate::str::contains("\"issues\": []"));
}

#[test]
fn lint_json_reports_missing_columns_as_issues_found() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("lint")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .arg("--against")
        .arg(fixture_path("datasets/valid/loan_tape_missing_rate.csv"))
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_issues_exit!(assert);

    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope.get("subcommand").and_then(|v| v.as_str()),
        Some("lint")
    );
    assert_eq!(
        envelope.get("outcome").and_then(|v| v.as_str()),
        Some("ISSUES_FOUND")
    );
    assert_eq!(envelope.get("exit_code").and_then(|v| v.as_i64()), Some(1));

    let issues = envelope
        .get("result")
        .and_then(|r| r.get("issues"))
        .and_then(|v| v.as_array())
        .expect("result.issues should be array");
    assert_eq!(issues.len(), 1);
    assert_eq!(
        issues[0].get("kind").and_then(|v| v.as_str()),
        Some("missing_column")
    );
    assert_eq!(
        issues[0].get("column").and_then(|v| v.as_str()),
        Some("rate")
    );
    assert_eq!(
        issues[0].get("severity").and_then(|v| v.as_str()),
        Some("error")
    );
}

#[test]
fn lint_json_issue_order_is_include_columns_then_key_columns() {
    let workspace = temp_workspace();
    let profile_path = workspace.path().join("lint_profile.yaml");
    fs::write(
        &profile_path,
        "\
schema_version: 1
status: draft
format: csv
equivalence:
  float_decimals: 6
  trim_strings: true
key:
  - missing_key
include_columns:
  - loan_id
  - missing_column
",
    )
    .expect("profile fixture write should succeed");

    let assert = profile_cmd()
        .arg("--json")
        .arg("lint")
        .arg(&profile_path)
        .arg("--against")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_issues_exit!(assert);

    let issues = envelope
        .get("result")
        .and_then(|r| r.get("issues"))
        .and_then(|v| v.as_array())
        .expect("result.issues should be array");
    assert_eq!(issues.len(), 2);

    assert_eq!(
        issues[0].get("kind").and_then(|v| v.as_str()),
        Some("missing_column")
    );
    assert_eq!(
        issues[0].get("column").and_then(|v| v.as_str()),
        Some("missing_column")
    );
    assert_eq!(
        issues[1].get("kind").and_then(|v| v.as_str()),
        Some("missing_key")
    );
    assert_eq!(
        issues[1].get("column").and_then(|v| v.as_str()),
        Some("missing_key")
    );
}

#[test]
fn lint_json_refuses_empty_dataset_with_e_empty() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("lint")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .arg("--against")
        .arg(fixture_path("datasets/invalid/empty.csv"))
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("code"))
            .and_then(|v| v.as_str()),
        Some("E_EMPTY")
    );
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("reason"))
            .and_then(|v| v.as_str()),
        Some("no header row")
    );
}

#[test]
fn lint_json_refuses_missing_dataset_path_with_e_io() {
    let workspace = temp_workspace();
    let missing_path = workspace.path().join("missing.csv");

    let assert = profile_cmd()
        .arg("--json")
        .arg("lint")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .arg("--against")
        .arg(&missing_path)
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("code"))
            .and_then(|v| v.as_str()),
        Some("E_IO")
    );
}

#[test]
fn lint_human_output_contract_for_refusal() {
    profile_cmd()
        .arg("lint")
        .arg(fixture_path("profiles/invalid/malformed_yaml.yaml"))
        .arg("--against")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .assert()
        .code(common::EXIT_REFUSAL)
        .stderr(predicate::str::contains("Error:"))
        .stderr(predicate::str::contains("Profile fails schema validation"));
}
