mod common;

use common::{assert_json_envelope_shape, fixture_path, parse_stdout_json, profile_cmd};
use predicates::prelude::predicate;

#[test]
fn validate_accepts_valid_draft_profile() {
    let assert = profile_cmd()
        .arg("validate")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .assert();
    common::assert_success_exit!(assert);
}

#[test]
fn validate_accepts_valid_frozen_profile() {
    let assert = profile_cmd()
        .arg("validate")
        .arg(fixture_path("profiles/valid/frozen_complete.yaml"))
        .assert();
    common::assert_success_exit!(assert);
}

#[test]
fn validate_json_success_uses_unified_output_envelope() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("validate")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope.get("subcommand").and_then(|v| v.as_str()),
        Some("validate")
    );
    assert_eq!(
        envelope.get("outcome").and_then(|v| v.as_str()),
        Some("SUCCESS")
    );
    assert_eq!(envelope.get("exit_code").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("valid"))
            .and_then(|v| v.as_bool()),
        Some(true)
    );
}

#[test]
fn validate_refuses_malformed_yaml_with_invalid_schema_code() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("validate")
        .arg(fixture_path("profiles/invalid/malformed_yaml.yaml"))
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
        Some("E_INVALID_SCHEMA")
    );
}

#[test]
fn validate_refuses_missing_field_with_expected_code() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("validate")
        .arg(fixture_path(
            "profiles/invalid/frozen_missing_profile_id.yaml",
        ))
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("code"))
            .and_then(|v| v.as_str()),
        Some("E_MISSING_FIELD")
    );
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("field"))
            .and_then(|v| v.as_str()),
        Some("profile_id")
    );
}

#[test]
fn validate_human_output_contracts_for_success_and_refusal() {
    profile_cmd()
        .arg("validate")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .assert()
        .code(common::EXIT_SUCCESS)
        .stdout(predicate::str::contains("✓ Profile is valid"));

    profile_cmd()
        .arg("validate")
        .arg(fixture_path(
            "profiles/invalid/frozen_missing_profile_id.yaml",
        ))
        .assert()
        .code(common::EXIT_REFUSAL)
        .stderr(predicate::str::contains("Error:"))
        .stderr(predicate::str::contains("Required field not declared"));
}
