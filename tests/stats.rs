mod common;

use std::fs;

use common::{
    assert_json_envelope_shape, copy_fixture, fixture_path, parse_stdout_json, profile_cmd,
    temp_workspace,
};

#[test]
fn stats_json_is_deterministic_for_full_dataset() {
    let assert_one = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("stats")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .assert();
    let envelope_one = parse_stdout_json(&assert_one);
    common::assert_success_exit!(assert_one);

    let assert_two = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("stats")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .assert();
    let envelope_two = parse_stdout_json(&assert_two);
    common::assert_success_exit!(assert_two);

    assert_eq!(envelope_one.get("result"), envelope_two.get("result"));
    assert_json_envelope_shape(&envelope_one);
    assert_eq!(
        envelope_one.get("subcommand").and_then(|v| v.as_str()),
        Some("stats")
    );
    assert_eq!(
        envelope_one
            .get("result")
            .and_then(|r| r.get("row_count"))
            .and_then(|v| v.as_i64()),
        Some(3)
    );
    assert_eq!(
        envelope_one
            .get("result")
            .and_then(|r| r.get("column_count"))
            .and_then(|v| v.as_i64()),
        Some(4)
    );

    let columns = envelope_one
        .get("result")
        .and_then(|r| r.get("columns"))
        .and_then(|v| v.as_array())
        .expect("stats result should contain columns array");
    let names = columns
        .iter()
        .map(|column| {
            column
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["loan_id", "balance", "rate", "property_type"]);
}

#[test]
fn stats_json_redacts_examples_by_default() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("stats")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    let columns = envelope
        .get("result")
        .and_then(|r| r.get("columns"))
        .and_then(|v| v.as_array())
        .expect("stats result should contain columns array");
    assert!(
        columns.iter().all(|column| column.get("example").is_none()),
        "stats should omit example fields unless --explicit is set"
    );
}

#[test]
fn stats_json_includes_examples_when_explicit() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("--explicit")
        .arg("stats")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    let columns = envelope
        .get("result")
        .and_then(|r| r.get("columns"))
        .and_then(|v| v.as_array())
        .expect("stats result should contain columns array");
    let loan_id = columns
        .iter()
        .find(|column| column.get("name").and_then(|v| v.as_str()) == Some("loan_id"))
        .expect("stats result should contain loan_id column");

    assert_eq!(
        loan_id.get("example").and_then(|value| value.as_str()),
        Some("LN-0001")
    );
}

#[test]
fn stats_json_respects_profile_scoping_and_column_order() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("stats")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .arg("--profile")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    let columns = envelope
        .get("result")
        .and_then(|r| r.get("columns"))
        .and_then(|v| v.as_array())
        .expect("stats result should contain columns array");
    let names = columns
        .iter()
        .map(|column| {
            column
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["loan_id", "balance", "rate"]);
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("column_count"))
            .and_then(|v| v.as_i64()),
        Some(3)
    );
}

#[test]
fn stats_json_refuses_when_profile_column_is_missing() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("stats")
        .arg(fixture_path("datasets/valid/loan_tape_missing_rate.csv"))
        .arg("--profile")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("code"))
            .and_then(|v| v.as_str()),
        Some("E_COLUMN_NOT_FOUND")
    );
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("columns"))
            .and_then(|v| v.as_array())
            .and_then(|values| values.first())
            .and_then(|v| v.as_str()),
        Some("rate")
    );
}

#[test]
fn stats_json_refuses_malformed_dataset_with_e_csv_parse() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("stats")
        .arg(fixture_path("datasets/invalid/malformed_quotes.csv"))
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("code"))
            .and_then(|v| v.as_str()),
        Some("E_CSV_PARSE")
    );
}

#[test]
fn stats_json_refuses_header_only_dataset_with_e_empty() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("stats")
        .arg(fixture_path("datasets/invalid/header_only.csv"))
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

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
        Some("no data rows")
    );
}

#[test]
fn stats_json_uses_canonical_profile_columns_via_registry() {
    let workspace = temp_workspace();
    let registry_dir = workspace.path().join("registries").join("annex_columns_v0");
    copy_fixture(
        "registries/annex_columns_v0/registry.json",
        registry_dir.join("registry.json"),
    );
    copy_fixture(
        "registries/annex_columns_v0/aliases.json",
        registry_dir.join("aliases.json"),
    );

    let profile_path = workspace.path().join("profile.yaml");
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
  - current_balance
  - note_rate
",
    )
    .expect("profile fixture write should succeed");

    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("stats")
        .arg(fixture_path("datasets/valid/loan_tape_alt_headers.csv"))
        .arg("--profile")
        .arg(&profile_path)
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    let columns = envelope
        .get("result")
        .and_then(|r| r.get("columns"))
        .and_then(|v| v.as_array())
        .expect("stats result should contain columns array");
    let names = columns
        .iter()
        .map(|column| {
            column
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec!["loan_id_number", "current_balance", "note_rate"]
    );
}
