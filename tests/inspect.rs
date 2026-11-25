mod common;

use common::{fixture_path, parse_stdout_json, profile_cmd};
use serde_json::Value;

fn run(path: &str) -> Value {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("inspect")
        .arg(fixture_path(path))
        .assert();
    let output = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);
    output
}

#[test]
fn inspect_is_zero_retention_by_default_and_contains_numeric_summary() {
    let output = run("datasets/valid/loan_tape_basic.csv");
    assert_eq!(output["result"]["format"], "csv");
    assert_eq!(output["result"]["row_count"], 3);
    assert_eq!(output["result"]["suggestions"]["header_row"], 1);
    assert_eq!(output["result"]["suggestions"]["preamble_rows"], 0);

    let columns = output["result"]["columns"].as_array().unwrap();
    let balance = columns
        .iter()
        .find(|column| column["name"] == "balance")
        .unwrap();
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
