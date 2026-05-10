mod common;

use common::{fixture_path, parse_stdout_json, profile_cmd, temp_workspace};
use serde_json::{Value, json};
use std::fs;

#[test]
fn slice_profile_driven_preamble_skip_writes_clean_csv_and_manifest() {
    let workspace = temp_workspace();
    let out = workspace.path().join("clean.csv");
    let manifest = workspace.path().join("slice.manifest.json");
    let assert = profile_cmd()
        .arg("slice")
        .arg(fixture_path("slice/preamble.csv"))
        .arg("--profile-path")
        .arg(fixture_path("slice/preparse_profile.yaml"))
        .arg("--out")
        .arg(&out)
        .arg("--emit-manifest")
        .arg(&manifest)
        .arg("--json")
        .arg("--no-witness")
        .assert();
    let envelope = parse_stdout_json(&assert);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    common::assert_success_exit!(assert);

    let clean = fs::read_to_string(&out).expect("read clean csv");
    assert_eq!(
        clean,
        "Account ID,Account Name,Amount,Closed Date\n1001,Alice Example,12.50,2026-01-01\n1002,Bob Example,44.00,2026-01-02\n"
    );
    assert!(!stdout.contains("Alice Example"));
    assert_eq!(envelope["subcommand"], "slice");
    assert_eq!(envelope["result"]["rows"]["output_data_rows"], 2);
    assert_eq!(
        envelope["result"]["output_path"],
        json!(out.display().to_string())
    );

    let manifest_json: Value =
        serde_json::from_str(&fs::read_to_string(&manifest).expect("manifest")).expect("json");
    assert_eq!(manifest_json["schema"], "profile.slice_manifest.v1");
    assert!(
        manifest_json["preamble_rows"]
            .as_array()
            .is_some_and(|rows| !rows.is_empty())
    );
}

#[test]
fn slice_ad_hoc_multi_row_header_can_stream_csv_to_stdout() {
    let assert = profile_cmd()
        .arg("slice")
        .arg(fixture_path("slice/multi_header.csv"))
        .arg("--mode")
        .arg("multi_row_header")
        .arg("--header-rows")
        .arg("1,2")
        .arg("--data-starts-at")
        .arg("3")
        .arg("--header-merge")
        .arg("ffill_concat")
        .arg("--no-witness")
        .assert();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    common::assert_success_exit!(assert);

    assert_eq!(
        stdout,
        "Portfolio.ID,Portfolio.Name,Metric.Amount,Metric.Rate\n1001,Alice Example,12.50,0.40\n1002,Bob Example,44.00,0.35\n"
    );
}

#[test]
fn slice_units_manifest_captures_units_by_explicit_opt_in() {
    let workspace = temp_workspace();
    let out = workspace.path().join("units.clean.csv");
    let manifest = workspace.path().join("units.manifest.json");
    let assert = profile_cmd()
        .arg("slice")
        .arg(fixture_path("slice/units.csv"))
        .arg("--mode")
        .arg("preamble_with_units")
        .arg("--header-at-row")
        .arg("2")
        .arg("--unit-rows")
        .arg("3")
        .arg("--data-starts-at")
        .arg("4")
        .arg("--out")
        .arg(&out)
        .arg("--emit-manifest")
        .arg(&manifest)
        .arg("--json")
        .arg("--no-witness")
        .assert();
    common::assert_success_exit!(assert);

    let clean = fs::read_to_string(&out).expect("read clean csv");
    assert_eq!(
        clean,
        "Region,Month,Volume,Rate\nNorth,2026-01,10,0.40\nSouth,2026-02,12,0.35\n"
    );
    let manifest_json: Value =
        serde_json::from_str(&fs::read_to_string(&manifest).expect("manifest")).expect("json");
    assert_eq!(
        manifest_json["unit_row_values"],
        json!([["text", "date", "units", "%"]])
    );
}

#[test]
fn validate_rejects_non_contiguous_pre_parse_header_rows() {
    let workspace = temp_workspace();
    let profile = workspace.path().join("bad.yaml");
    fs::write(
        &profile,
        r#"schema_version: 1
status: draft
format: csv
pre_parse:
  slice:
    mode: multi_row_header
    header_rows: [1, 3]
    data_starts_at: 4
include_columns:
  - id
"#,
    )
    .expect("write bad profile");

    let assert = profile_cmd()
        .arg("validate")
        .arg(&profile)
        .arg("--json")
        .arg("--no-witness")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);
    assert_eq!(envelope["outcome"], "REFUSAL");
    assert_eq!(envelope["result"]["code"], "E_INVALID_SCHEMA");
}

#[test]
fn draft_init_from_peek_uses_suggested_slice_headers() {
    let workspace = temp_workspace();
    let peek = workspace.path().join("peek.json");
    let out = workspace.path().join("profile.yaml");
    fs::write(
        &peek,
        serde_json::to_string(&json!({
            "version": "fingerprint.peek.v0",
            "outcome": "SUCCESS",
            "result": {
                "summary": {
                    "modal_column_count": 4,
                    "data_starts_at": 5
                },
                "suggestions": {
                    "profile_pre_parse": {
                        "mode": "preamble_skip",
                        "skip_rows": 3,
                        "header_at_row": 4,
                        "unit_rows": [],
                        "data_starts_at": 5
                    }
                }
            }
        }))
        .expect("serialize peek"),
    )
    .expect("write peek");

    let assert = profile_cmd()
        .arg("draft")
        .arg("init")
        .arg(fixture_path("slice/preamble.csv"))
        .arg("--from-peek")
        .arg(&peek)
        .arg("--out")
        .arg(&out)
        .arg("--no-witness")
        .assert();
    common::assert_success_exit!(assert);

    let yaml: Value =
        serde_yaml::from_str(&fs::read_to_string(&out).expect("profile yaml")).expect("yaml");
    assert_eq!(yaml["include_columns"][0], "Account ID");
    assert_eq!(yaml["pre_parse"]["slice"]["header_at_row"], 4);
}
