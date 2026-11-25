mod common;

use std::fs;

use common::{fixture_path, parse_stdout_json, profile_cmd, temp_workspace};
use serde_json::Value;

#[test]
fn profile_v1_metadata_and_multiformat_profile_freeze_deterministically() {
    let workspace = temp_workspace();
    let draft = workspace.path().join("metadata.yaml");
    let one = workspace.path().join("one.yaml");
    let two = workspace.path().join("two.yaml");
    fs::write(
        &draft,
        "\
schema_version: 1
status: draft
format: parquet
pre_parse:
  sheet: loans
  fingerprint_extract: extract-v1
  slice:
    mode: preamble_skip
columns:
  loan_id:
    type: text
    nullable: false
    sensitivity: protected
    airlock_class: forbidden
  balance:
    type: number
    nullable: true
    unit: USD
    sensitivity: derived
key:
  - loan_id
include_columns:
  - loan_id
  - balance
",
    )
    .unwrap();

    profile_cmd()
        .arg("validate")
        .arg(&draft)
        .assert()
        .code(common::EXIT_SUCCESS);

    for out in [&one, &two] {
        profile_cmd()
            .arg("--no-witness")
            .arg("freeze")
            .arg(&draft)
            .arg("--family")
            .arg("loan.metadata")
            .arg("--version")
            .arg("1")
            .arg("--out")
            .arg(out)
            .assert()
            .code(common::EXIT_SUCCESS);
    }
    assert_eq!(fs::read(&one).unwrap(), fs::read(&two).unwrap());
    let frozen: Value = serde_yaml::from_slice(&fs::read(&one).unwrap()).unwrap();
    assert_eq!(frozen["format"], "parquet");
    assert_eq!(frozen["columns"]["balance"]["unit"], "USD");
    assert_eq!(frozen["pre_parse"]["sheet"], "loans");
}

#[test]
fn metadata_validation_rejects_blank_units_and_unknown_sensitivity() {
    let workspace = temp_workspace();
    let blank = workspace.path().join("blank-unit.yaml");
    fs::write(
        &blank,
        "schema_version: 1\nstatus: draft\nformat: csv\ncolumns:\n  balance:\n    unit: '   '\ninclude_columns:\n  - balance\n",
    )
    .unwrap();
    let blank_result = profile_cmd()
        .arg("--json")
        .arg("validate")
        .arg(&blank)
        .assert();
    let blank_json = parse_stdout_json(&blank_result);
    common::assert_refusal_exit!(blank_result);
    assert_eq!(blank_json["result"]["code"], "E_INVALID_SCHEMA");
    assert_eq!(
        blank_json["result"]["detail"]["errors"][0]["field"],
        "columns.balance.unit"
    );

    let unknown = workspace.path().join("unknown-sensitivity.yaml");
    fs::write(
        &unknown,
        "schema_version: 1\nstatus: draft\nformat: jsonl\ncolumns:\n  balance:\n    sensitivity: secret\ninclude_columns:\n  - balance\n",
    )
    .unwrap();
    let unknown_result = profile_cmd()
        .arg("--json")
        .arg("validate")
        .arg(&unknown)
        .assert();
    let unknown_json = parse_stdout_json(&unknown_result);
    common::assert_refusal_exit!(unknown_result);
    assert_eq!(unknown_json["result"]["code"], "E_INVALID_SCHEMA");
}

#[test]
fn stats_and_draft_init_infer_metadata_without_retaining_values() {
    let workspace = temp_workspace();
    let dataset = fixture_path("datasets/valid/loan_tape_basic.csv");
    // bd-ds5: default stats withholds header text (an ordinal ref `id`); --explicit names it.
    let redacted = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("stats")
        .arg(&dataset)
        .assert();
    let redacted_json = parse_stdout_json(&redacted);
    common::assert_success_exit!(redacted);
    assert_eq!(
        redacted_json["result"]["columns"][0]["id"],
        "redacted:ref:c1"
    );
    assert!(!redacted_json.to_string().contains("loan_id"));
    let stats = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("--explicit")
        .arg("stats")
        .arg(&dataset)
        .assert();
    let stats_json = parse_stdout_json(&stats);
    common::assert_success_exit!(stats);
    let loan_id = stats_json["result"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|column| column["name"] == "loan_id")
        .unwrap();
    assert_eq!(loan_id["type"], "text");
    assert_eq!(loan_id["nullable"], false);
    assert!(stats_json.to_string().contains("\"type\""));
    // Values stay out of the default output (--explicit adds an example, so check the default run).
    assert!(!redacted_json.to_string().contains("LN-0001"));

    let draft = workspace.path().join("draft.yaml");
    profile_cmd()
        .arg("draft")
        .arg("init")
        .arg(&dataset)
        .arg("--out")
        .arg(&draft)
        .assert()
        .code(common::EXIT_SUCCESS);
    let yaml: Value = serde_yaml::from_slice(&fs::read(&draft).unwrap()).unwrap();
    assert_eq!(yaml["columns"]["balance"]["type"], "number");
    assert_eq!(yaml["columns"]["balance"]["nullable"], false);
}

#[test]
fn frozen_profile_projections_use_stats_and_never_emit_raw_values() {
    let workspace = temp_workspace();
    let stats_path = workspace.path().join("stats.json");
    let stats = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("--explicit")
        .arg("stats")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .assert();
    let stats_stdout = stats.get_output().stdout.clone();
    common::assert_success_exit!(stats);
    fs::write(&stats_path, stats_stdout).unwrap();

    let profile = fixture_path("profiles/valid/frozen_complete.yaml");
    let verify = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("emit-verify-starter")
        .arg(&profile)
        .arg("--stats")
        .arg(&stats_path)
        .assert();
    let verify_json = parse_stdout_json(&verify);
    common::assert_success_exit!(verify);
    // bd-hoh: the result is verify portable authoring (no `schema` field); the stats feed it
    // through nullability: stats-derived non-null columns become `not_null` rules. Column types
    // have no portable op and are no longer carried.
    assert!(verify_json["result"].get("schema").is_none());
    assert_eq!(
        verify_json["result"]["constraint_set_id"],
        "csv.loan_tape.core.v0"
    );
    assert_eq!(verify_json["result"]["rules"][0]["op"], "unique");
    assert!(
        verify_json["result"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|rule| rule["op"] == "not_null"
                && rule["columns"] == serde_json::json!(["balance"]))
    );
    assert!(!verify_json.to_string().contains("LN-0001"));

    let ddl = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("emit-ddl")
        .arg(&profile)
        .arg("--stats")
        .arg(&stats_path)
        .arg("--dialect")
        .arg("snowflake")
        .arg("--table")
        .arg("loan_tape")
        .assert();
    let ddl_json = parse_stdout_json(&ddl);
    common::assert_success_exit!(ddl);
    assert_eq!(ddl_json["result"]["schema"], "twinning.ddl.v1");
    assert!(
        ddl_json["result"]["sql"]
            .as_str()
            .unwrap()
            .contains("CREATE TABLE \"loan_tape\"")
    );
    assert!(!ddl_json.to_string().contains("LN-0001"));
}

#[test]
fn projections_refuse_default_redacted_stats_instead_of_silently_dropping_types() {
    // bd-ds5: default `profile stats` carries column `id`s, not names. Feeding that file to a
    // projection used to skip every column silently; it now refuses and says how to fix it.
    let workspace = temp_workspace();
    let stats_path = workspace.path().join("stats.json");
    let stats = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("stats")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .assert();
    let stats_stdout = stats.get_output().stdout.clone();
    common::assert_success_exit!(stats);
    fs::write(&stats_path, stats_stdout).unwrap();
    for command in ["emit-verify-starter", "emit-ddl"] {
        let mut cmd = profile_cmd();
        cmd.arg("--json")
            .arg("--no-witness")
            .arg(command)
            .arg(fixture_path("profiles/valid/frozen_complete.yaml"))
            .arg("--stats")
            .arg(&stats_path);
        if command == "emit-ddl" {
            cmd.args(["--dialect", "snowflake", "--table", "loan_tape"]);
        }
        let assert = cmd.assert();
        let json = parse_stdout_json(&assert);
        common::assert_refusal_exit!(assert);
        assert_eq!(json["result"]["code"], "E_INVALID_SCHEMA", "{command}");
        assert!(json.to_string().contains("--explicit"), "{command}: {json}");
    }
}
