mod common;

use common::{
    assert_json_envelope_shape, fixture_path, parse_stdout_json, profile_cmd, temp_workspace,
};

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

fn assert_refusal_code(cmd: &mut assert_cmd::Command, expected_code: &str) {
    let assert = cmd.assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

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
