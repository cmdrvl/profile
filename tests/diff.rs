mod common;

use std::fs;

use common::{
    assert_json_envelope_shape, copy_fixture, parse_stdout_json, profile_cmd, temp_workspace,
};

#[test]
fn diff_json_exits_zero_for_semantic_equivalence_with_identity_changes_only() {
    let workspace = temp_workspace();
    let profile_a = copy_fixture(
        "profiles/valid/frozen_complete.yaml",
        workspace.path().join("a.yaml"),
    );
    let profile_b = workspace.path().join("b.yaml");
    fs::write(
        &profile_b,
        "\
schema_version: 1
profile_id: csv.loan_tape.alt.v42
profile_version: 42
profile_family: csv.loan_tape.alt
profile_sha256: sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
status: frozen
format: csv
hashing:
  algorithm: sha256
equivalence:
  order: order-invariant
  float_decimals: 6
  trim_strings: true
key:
  - loan_id
include_columns:
  - loan_id
  - balance
  - rate
",
    )
    .expect("comparison profile should be written");

    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("diff")
        .arg(&profile_a)
        .arg(&profile_b)
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope.get("outcome").and_then(|v| v.as_str()),
        Some("SUCCESS")
    );
    assert_eq!(envelope.get("exit_code").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("equivalent"))
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("differences"))
            .and_then(|v| v.as_array())
            .map(Vec::len),
        Some(0)
    );
}

#[test]
fn diff_json_exits_one_with_deterministic_semantic_difference_order() {
    let workspace = temp_workspace();
    let profile_a = copy_fixture(
        "profiles/valid/frozen_complete.yaml",
        workspace.path().join("a.yaml"),
    );
    let profile_b = workspace.path().join("b-semantic-diff.yaml");
    fs::write(
        &profile_b,
        "\
schema_version: 1
profile_id: csv.loan_tape.diff.v1
profile_version: 1
profile_family: csv.loan_tape.diff
profile_sha256: sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
status: frozen
format: csv
key:
  - balance
include_columns:
  - loan_id
  - balance
",
    )
    .expect("comparison profile should be written");

    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("diff")
        .arg(&profile_a)
        .arg(&profile_b)
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_issues_exit!(assert);

    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope.get("outcome").and_then(|v| v.as_str()),
        Some("ISSUES_FOUND")
    );
    assert_eq!(envelope.get("exit_code").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("equivalent"))
            .and_then(|v| v.as_bool()),
        Some(false)
    );

    let differences = envelope
        .get("result")
        .and_then(|r| r.get("differences"))
        .and_then(|v| v.as_array())
        .expect("result.differences should be array");
    let fields = differences
        .iter()
        .map(|difference| {
            difference
                .get("field")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        fields,
        vec!["hashing", "equivalence", "key", "include_columns"]
    );
    assert!(
        !fields.contains(&"profile_id"),
        "identity fields must be excluded from semantic diff"
    );
}
