mod common;

use common::{
    assert_json_envelope_shape, fixture_path, parse_stdout_json, profile_cmd, temp_workspace,
};
use predicates::str::contains;
use serde_json::Value;

#[test]
fn refusal_codes_emit_unified_json_envelope() {
    let workspace = temp_workspace();
    let bad_version_out = workspace.path().join("bad-version.yaml");
    let already_frozen_out = workspace.path().join("already-frozen.yaml");

    let mut invalid_schema_cmd = profile_cmd();
    invalid_schema_cmd
        .arg("--json")
        .arg("validate")
        .arg(fixture_path("profiles/invalid/frozen_bad_sha.yaml"));
    assert_refusal_code(&mut invalid_schema_cmd, "E_INVALID_SCHEMA");

    let mut missing_field_cmd = profile_cmd();
    missing_field_cmd
        .arg("--json")
        .arg("validate")
        .arg(fixture_path(
            "profiles/invalid/frozen_missing_profile_id.yaml",
        ));
    assert_refusal_code(&mut missing_field_cmd, "E_MISSING_FIELD");

    let mut bad_version_cmd = profile_cmd();
    bad_version_cmd
        .arg("--json")
        .arg("freeze")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .arg("--family")
        .arg("Csv..bad")
        .arg("--version")
        .arg("0")
        .arg("--out")
        .arg(&bad_version_out);
    assert_refusal_code(&mut bad_version_cmd, "E_BAD_VERSION");

    let mut already_frozen_cmd = profile_cmd();
    already_frozen_cmd
        .arg("--json")
        .arg("freeze")
        .arg(fixture_path("profiles/valid/frozen_complete.yaml"))
        .arg("--family")
        .arg("csv.loan_tape.core")
        .arg("--version")
        .arg("0")
        .arg("--out")
        .arg(&already_frozen_out);
    assert_refusal_code(&mut already_frozen_cmd, "E_ALREADY_FROZEN");

    let mut io_cmd = profile_cmd();
    io_cmd
        .arg("--json")
        .arg("validate")
        .arg(workspace.path().join("missing-file.yaml"));
    assert_refusal_code(&mut io_cmd, "E_IO");

    let mut csv_parse_cmd = profile_cmd();
    csv_parse_cmd
        .arg("--json")
        .arg("stats")
        .arg(fixture_path("datasets/invalid/malformed_quotes.csv"));
    assert_refusal_code(&mut csv_parse_cmd, "E_CSV_PARSE");

    let mut empty_cmd = profile_cmd();
    empty_cmd
        .arg("--json")
        .arg("suggest-key")
        .arg(fixture_path("datasets/invalid/header_only.csv"));
    assert_refusal_code(&mut empty_cmd, "E_EMPTY");

    let mut column_missing_cmd = profile_cmd();
    column_missing_cmd
        .arg("--json")
        .arg("stats")
        .arg(fixture_path("datasets/valid/loan_tape_missing_rate.csv"))
        .arg("--profile")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"));
    assert_refusal_code(&mut column_missing_cmd, "E_COLUMN_NOT_FOUND");
}

#[test]
fn refusal_detail_payloads_match_plan_contract() {
    let workspace = temp_workspace();
    let bad_version_out = workspace.path().join("detail-bad-version.yaml");
    let already_frozen_out = workspace.path().join("detail-already-frozen.yaml");
    let missing_file = workspace.path().join("missing-file.yaml");

    let mut invalid_schema_cmd = profile_cmd();
    invalid_schema_cmd
        .arg("--json")
        .arg("validate")
        .arg(fixture_path("profiles/invalid/frozen_bad_sha.yaml"));
    let invalid_schema = refusal_envelope(&mut invalid_schema_cmd);
    assert_eq!(refusal_code(&invalid_schema), Some("E_INVALID_SCHEMA"));
    assert!(
        invalid_schema
            .get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("errors"))
            .and_then(|e| e.as_array())
            .is_some_and(|errors| !errors.is_empty())
    );

    let mut missing_field_cmd = profile_cmd();
    missing_field_cmd
        .arg("--json")
        .arg("validate")
        .arg(fixture_path(
            "profiles/invalid/frozen_missing_profile_id.yaml",
        ));
    let missing_field = refusal_envelope(&mut missing_field_cmd);
    assert_eq!(refusal_code(&missing_field), Some("E_MISSING_FIELD"));
    assert_eq!(
        missing_field
            .get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("field"))
            .and_then(|f| f.as_str()),
        Some("profile_id")
    );

    let mut bad_version_cmd = profile_cmd();
    bad_version_cmd
        .arg("--json")
        .arg("freeze")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .arg("--family")
        .arg("Csv..bad")
        .arg("--version")
        .arg("0")
        .arg("--out")
        .arg(&bad_version_out);
    let bad_version = refusal_envelope(&mut bad_version_cmd);
    assert_eq!(refusal_code(&bad_version), Some("E_BAD_VERSION"));
    assert_eq!(
        bad_version
            .get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("family"))
            .and_then(|f| f.as_str()),
        Some("Csv..bad")
    );
    assert!(
        bad_version
            .get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("error"))
            .and_then(|e| e.as_str())
            .is_some()
    );

    let mut already_frozen_cmd = profile_cmd();
    already_frozen_cmd
        .arg("--json")
        .arg("freeze")
        .arg(fixture_path("profiles/valid/frozen_complete.yaml"))
        .arg("--family")
        .arg("csv.loan_tape.core")
        .arg("--version")
        .arg("0")
        .arg("--out")
        .arg(&already_frozen_out);
    let already_frozen = refusal_envelope(&mut already_frozen_cmd);
    assert_eq!(refusal_code(&already_frozen), Some("E_ALREADY_FROZEN"));
    assert!(
        already_frozen
            .get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("profile_id"))
            .and_then(|id| id.as_str())
            .is_some()
    );
    assert!(
        already_frozen
            .get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("profile_sha256"))
            .and_then(|sha| sha.as_str())
            .is_some()
    );

    let mut io_cmd = profile_cmd();
    io_cmd.arg("--json").arg("validate").arg(&missing_file);
    let io = refusal_envelope(&mut io_cmd);
    assert_eq!(refusal_code(&io), Some("E_IO"));
    assert_eq!(
        io.get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("path"))
            .and_then(|p| p.as_str()),
        missing_file.to_str()
    );
    assert!(
        io.get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("error"))
            .and_then(|e| e.as_str())
            .is_some()
    );

    let mut csv_parse_cmd = profile_cmd();
    csv_parse_cmd
        .arg("--json")
        .arg("stats")
        .arg(fixture_path("datasets/invalid/malformed_quotes.csv"));
    let csv_parse = refusal_envelope(&mut csv_parse_cmd);
    assert_eq!(refusal_code(&csv_parse), Some("E_CSV_PARSE"));
    assert!(
        csv_parse
            .get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("path"))
            .and_then(|p| p.as_str())
            .is_some()
    );
    assert!(
        csv_parse
            .get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("error"))
            .and_then(|e| e.as_str())
            .is_some()
    );

    let mut empty_cmd = profile_cmd();
    empty_cmd
        .arg("--json")
        .arg("suggest-key")
        .arg(fixture_path("datasets/invalid/header_only.csv"));
    let empty = refusal_envelope(&mut empty_cmd);
    assert_eq!(refusal_code(&empty), Some("E_EMPTY"));
    assert!(
        empty
            .get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("path"))
            .and_then(|p| p.as_str())
            .is_some()
    );
    assert_eq!(
        empty
            .get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("reason"))
            .and_then(|r| r.as_str()),
        Some("no data rows")
    );

    let mut column_not_found_cmd = profile_cmd();
    column_not_found_cmd
        .arg("--json")
        .arg("stats")
        .arg(fixture_path("datasets/valid/loan_tape_missing_rate.csv"))
        .arg("--profile")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"));
    let column_not_found = refusal_envelope(&mut column_not_found_cmd);
    assert_eq!(refusal_code(&column_not_found), Some("E_COLUMN_NOT_FOUND"));
    assert!(
        column_not_found
            .get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("columns"))
            .and_then(|c| c.as_array())
            .is_some_and(|columns| !columns.is_empty())
    );
    assert!(
        column_not_found
            .get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("available"))
            .and_then(|a| a.as_array())
            .is_some_and(|available| !available.is_empty())
    );
}

#[test]
fn human_refusal_includes_code_in_brackets() {
    profile_cmd()
        .arg("lint")
        .arg(fixture_path("profiles/invalid/malformed_yaml.yaml"))
        .arg("--against")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .assert()
        .code(common::EXIT_REFUSAL)
        .stderr(contains("Error: [E_INVALID_SCHEMA]"))
        .stderr(contains("Profile fails schema validation"));
}

fn assert_refusal_code(cmd: &mut assert_cmd::Command, expected_code: &str) {
    let envelope = refusal_envelope(cmd);
    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope.get("outcome").and_then(|v| v.as_str()),
        Some("REFUSAL")
    );
    assert_eq!(envelope.get("exit_code").and_then(|v| v.as_i64()), Some(2));
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("code"))
            .and_then(|v| v.as_str()),
        Some(expected_code)
    );
}

fn refusal_envelope(cmd: &mut assert_cmd::Command) -> Value {
    let assert = cmd.assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);
    envelope
}

fn refusal_code(envelope: &Value) -> Option<&str> {
    envelope
        .get("result")
        .and_then(|r| r.get("code"))
        .and_then(|v| v.as_str())
}
