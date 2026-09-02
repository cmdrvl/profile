mod common;

use std::fs;

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
fn validate_accepts_ordered_composite_key_fixture() {
    let assert = profile_cmd()
        .arg("validate")
        .arg(fixture_path("profiles/valid/draft_composite_key.yaml"))
        .assert();
    common::assert_success_exit!(assert);
}

#[test]
fn validate_rejects_empty_and_duplicate_key_components_deterministically() {
    let workspace = common::temp_workspace();
    let cases = [
        (
            "empty-key.yaml",
            "  - loan_id\n  - '   '\n",
            "key[1]",
            "must be a non-empty string",
        ),
        (
            "duplicate-key.yaml",
            "  - loan_id\n  - property_type\n  - loan_id\n",
            "key[2]",
            "duplicate key column 'loan_id' first declared at key[0]",
        ),
    ];

    for (file_name, key_yaml, expected_field, expected_error) in cases {
        let profile_path = workspace.path().join(file_name);
        fs::write(
            &profile_path,
            format!(
                "schema_version: 1\nstatus: draft\nformat: csv\nkey:\n{key_yaml}include_columns:\n  - loan_id\n"
            ),
        )
        .expect("invalid key profile fixture should be written");

        let assert = profile_cmd()
            .arg("--json")
            .arg("validate")
            .arg(&profile_path)
            .assert();
        let envelope = parse_stdout_json(&assert);
        common::assert_refusal_exit!(assert);

        assert_eq!(envelope["result"]["code"], "E_INVALID_SCHEMA");
        assert_eq!(
            envelope["result"]["detail"]["errors"][0]["field"],
            expected_field
        );
        assert_eq!(
            envelope["result"]["detail"]["errors"][0]["error"],
            expected_error
        );
    }
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

#[test]
fn validate_accepts_profile_with_column_registry() {
    let workspace = common::temp_workspace();
    let profile_path = workspace.path().join("with-registry.yaml");
    fs::write(
        &profile_path,
        "\
schema_version: 1
status: draft
format: csv
column_registry: registries/annex_columns_v0
key:
  - loan_id_number
include_columns:
  - loan_id_number
",
    )
    .expect("profile fixture write should succeed");

    let assert = profile_cmd().arg("validate").arg(&profile_path).assert();
    common::assert_success_exit!(assert);
}

#[test]
fn validate_accepts_supported_slice_encoding_labels() {
    let workspace = common::temp_workspace();

    for encoding in ["utf-8", "windows-1252", "latin1"] {
        let profile_path = workspace
            .path()
            .join(format!("with-{}.yaml", encoding.replace('-', "_")));
        fs::write(
            &profile_path,
            format!(
                "\
schema_version: 1
status: draft
format: csv
pre_parse:
  slice:
    mode: preamble_skip
    header_at_row: 1
    data_starts_at: 2
    encoding: {encoding}
include_columns:
  - id
"
            ),
        )
        .expect("profile fixture write should succeed");

        let assert = profile_cmd()
            .arg("--json")
            .arg("--no-witness")
            .arg("validate")
            .arg(&profile_path)
            .assert();
        common::assert_success_exit!(assert);
    }
}

#[test]
fn validate_refuses_unsupported_slice_encoding_label() {
    let workspace = common::temp_workspace();
    let profile_path = workspace.path().join("unsupported-encoding.yaml");
    fs::write(
        &profile_path,
        "\
schema_version: 1
status: draft
format: csv
pre_parse:
  slice:
    mode: preamble_skip
    header_at_row: 1
    data_starts_at: 2
    encoding: utf-16
include_columns:
  - id
",
    )
    .expect("profile fixture write should succeed");

    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("validate")
        .arg(&profile_path)
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);
    assert_eq!(envelope["result"]["code"], "E_INVALID_SCHEMA");
    assert_eq!(
        envelope["result"]["detail"]["errors"][0]["field"],
        "pre_parse.slice.encoding"
    );
}
