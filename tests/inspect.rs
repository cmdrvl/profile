mod common;

use common::{fixture_path, parse_stdout_json, profile_cmd};
use serde_json::Value;
use std::fs;

fn run(path: &str) -> Value {
    run_with(path, &[])
}

fn run_with(path: &str, flags: &[&str]) -> Value {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .args(flags)
        .arg("inspect")
        .arg(fixture_path(path))
        .assert();
    let output = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);
    output
}

#[test]
fn inspect_default_redacts_extrema_and_preserves_numeric_summary() {
    let output = run("datasets/valid/loan_tape_basic.csv");
    assert_eq!(output["result"]["format"], "csv");
    assert_eq!(output["result"]["row_count"], 3);
    assert_eq!(output["result"]["suggestions"]["header_row"], 1);
    assert_eq!(output["result"]["suggestions"]["preamble_rows"], 0);

    // bd-ds5: header text is never printed by default (it can be a banner or a data row): a
    // column is an ordinal ref. --explicit adds the name, so the same stats are looked up by name.
    let default_stdout = output.to_string();
    assert!(!default_stdout.contains("balance") && !default_stdout.contains("loan_id"));
    assert_eq!(output["result"]["columns"][0]["id"], "redacted:ref:c1");
    assert!(output["result"]["columns"][0].get("name").is_none());
    let named = run_with("datasets/valid/loan_tape_basic.csv", &["--explicit"]);
    let columns = named["result"]["columns"].as_array().unwrap();
    let balance = columns
        .iter()
        .find(|column| column["name"] == "balance")
        .unwrap();
    let default_balance = output["result"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|column| column["id"] == balance["id"])
        .unwrap();
    assert_eq!(default_balance["numeric"]["min"], Value::Null);
    assert_eq!(default_balance["numeric"]["max"], Value::Null);
    assert_eq!(default_balance["numeric"]["p50"], balance["numeric"]["p50"]);
    assert_eq!(default_balance["numeric"]["p95"], balance["numeric"]["p95"]);
    assert_eq!(balance["type"], "number");
    assert_eq!(balance["null_count"], 0);
    assert_eq!(balance["distinct_count"], 3);
    assert_eq!(balance["numeric"]["min"], 98000.0);
    assert_eq!(balance["numeric"]["max"], 125000.0);
    assert_eq!(balance["numeric"]["p50"], 100000.0);
    assert_eq!(balance["numeric"]["p95"], 122500.0);
    assert!(!output.to_string().contains("LN-0001"));
    assert!(!output.to_string().contains("SFR"));
}

#[test]
fn inspect_is_deterministic_across_supported_tabular_containers() {
    let baseline = run("formats/old.csv");
    for extension in ["tsv", "jsonl", "parquet", "xlsx"] {
        let got = run(&format!("formats/old.{extension}"));
        for column in got["result"]["columns"].as_array().unwrap() {
            if column["numeric"].is_object() {
                assert_eq!(column["numeric"]["min"], Value::Null, "old.{extension}");
                assert_eq!(column["numeric"]["max"], Value::Null, "old.{extension}");
            }
        }
        let explicit = run_with(&format!("formats/old.{extension}"), &["--explicit"]);
        let balance = explicit["result"]["columns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|column| column["name"] == "balance")
            .unwrap();
        assert_eq!(balance["numeric"]["min"], 300.25, "old.{extension}");
        assert_eq!(balance["numeric"]["max"], 2000.0, "old.{extension}");
        let mut expected_result = baseline["result"].clone();
        let mut actual_result = got["result"].clone();
        for result in [&mut expected_result, &mut actual_result] {
            result.as_object_mut().unwrap().remove("format");
            result.as_object_mut().unwrap().remove("dialect");
        }
        assert_eq!(actual_result, expected_result, "old.{extension}");
    }
}

#[test]
fn inspect_explicit_opt_in_is_the_only_way_to_emit_an_example() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("--explicit")
        .arg("inspect")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .assert();
    let output = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);
    assert_eq!(output["result"]["columns"][0]["example"], "LN-0001");
}

#[test]
fn inspect_refuses_header_only_input_without_partial_metadata() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("inspect")
        .arg(fixture_path("datasets/invalid/header_only.csv"))
        .assert();
    let output = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);
    assert_eq!(output["result"]["code"], "E_EMPTY");
    assert!(!output.to_string().contains("loan_id"));
}

#[test]
fn inspect_default_redacts_numeric_extrema_canary() {
    let workspace = common::temp_workspace();
    let dataset = workspace.path().join("numeric-canary.csv");
    fs::write(&dataset, "id,amount\nalpha,918273645.19\nbeta,10\n").unwrap();

    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("inspect")
        .arg(&dataset)
        .assert();
    let output = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    let amount = output["result"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|column| column["id"] == "redacted:ref:c2")
        .unwrap();
    assert_eq!(amount["numeric"]["min"], Value::Null);
    assert_eq!(amount["numeric"]["max"], Value::Null);
    assert_eq!(amount["numeric"]["p50"], 459136827.595);
    let rendered = output.to_string();
    assert!(!rendered.contains("918273645.19"));
    assert!(!rendered.contains("\"min\":10"));
}

#[test]
fn inspect_human_output_redacts_numeric_extrema() {
    let workspace = common::temp_workspace();
    let dataset = workspace.path().join("numeric-canary.csv");
    fs::write(&dataset, "id,amount\nalpha,918273645.19\nbeta,10\n").unwrap();

    let assert = profile_cmd().arg("inspect").arg(&dataset).assert();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    common::assert_success_exit!(assert);
    assert!(!stdout.contains("918273645.19"));
    assert!(!stdout.contains("\"min\": 10"));
    assert!(!stdout.contains("\"max\": 918273645.19"));
}

#[test]
fn inspect_errors_do_not_echo_numeric_canary() {
    let workspace = common::temp_workspace();
    let dataset = workspace.path().join("malformed-canary.csv");
    fs::write(
        &dataset,
        "id,amount\nalpha,918273645.19\n\"unterminated,10\n",
    )
    .unwrap();

    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("inspect")
        .arg(&dataset)
        .assert();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    common::assert_refusal_exit!(assert);
    assert!(!stdout.contains("918273645.19"));
    assert!(!stderr.contains("918273645.19"));
}

#[test]
fn inspect_does_not_write_a_witness_record() {
    let workspace = common::temp_workspace();
    let dataset = workspace.path().join("numeric-canary.csv");
    let ledger = workspace.path().join("witness.jsonl");
    fs::write(&dataset, "id,amount\nalpha,918273645.19\nbeta,10\n").unwrap();

    let assert = profile_cmd()
        .env("EPISTEMIC_WITNESS", &ledger)
        .arg("--json")
        .arg("inspect")
        .arg(&dataset)
        .assert();
    let output = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);
    assert!(output["witness_id"].is_null());
    assert!(!ledger.exists());
}
