mod common;

use common::{fixture_path, parse_stdout_json, profile_cmd, temp_workspace};
use serde_json::Value;
use std::fs;

#[test]
fn emit_discovery_emits_linkedin_candidate_template() {
    let assert = profile_cmd()
        .arg("emit-discovery")
        .arg(fixture_path("discovery/linkedin_sliced.csv"))
        .arg("--source-file")
        .arg(fixture_path("discovery/linkedin_source.csv"))
        .arg("--skip-rows")
        .arg("3")
        .arg("--source-kind")
        .arg("linkedin_export")
        .arg("--json")
        .arg("--no-witness")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    common::assert_json_envelope_shape(&envelope);
    assert_eq!(envelope["subcommand"], "emit-discovery");
    assert_eq!(envelope["outcome"], "SUCCESS");
    assert_eq!(envelope["witness_id"], Value::Null);

    let result = &envelope["result"];
    assert_eq!(result["version"], "profile.discovery.v0");
    assert_eq!(result["outcome"], "DISCOVERED");
    assert_eq!(
        result["candidate_template"]["id"],
        "linkedin_export.candidate.v0"
    );
    assert_eq!(
        result["candidate_template"]["source_kind"],
        "linkedin_export"
    );
    assert_eq!(result["candidate_template"]["skip_rows"], 3);
    assert_eq!(result["candidate_template"]["header_row_offset"], 3);
    assert_eq!(result["candidate_template"]["column_count"], 7);
    assert_eq!(
        result["candidate_template"]["headers"]
            .as_array()
            .expect("headers array")
            .len(),
        7
    );
    assert_eq!(result["candidate_template"]["evidence"]["lines_scanned"], 6);
    assert_eq!(
        result["candidate_template"]["evidence"]["consistent_column_count_below_offset"],
        true
    );
    assert_eq!(
        result["candidate_template"]["evidence"]["preamble_lines"]
            .as_array()
            .expect("preamble lines")
            .len(),
        3
    );

    let sha = result["candidate_template"]["evidence"]["source_file_sha256"]
        .as_str()
        .expect("source hash string");
    assert!(sha.starts_with("sha256:"));
    assert_eq!(sha.len(), 71);
}

#[test]
fn emit_discovery_output_is_byte_identical_for_same_inputs() {
    let args = [
        "emit-discovery",
        "tests/fixtures/discovery/linkedin_sliced.csv",
        "--source-file",
        "tests/fixtures/discovery/linkedin_source.csv",
        "--skip-rows",
        "3",
        "--source-kind",
        "linkedin_export",
        "--json",
        "--no-witness",
    ];

    let first_stdout = profile_cmd()
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let second_stdout = profile_cmd()
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(first_stdout, second_stdout);
}

#[test]
fn emit_discovery_supports_gmail_and_generic_fixtures() {
    let cases = [
        (
            "gmail",
            "discovery/gmail_sliced.csv",
            "discovery/gmail_source.csv",
            "gmail_address_book",
            "1",
            4,
        ),
        (
            "generic",
            "discovery/generic_sliced.csv",
            "discovery/generic_source.csv",
            "generic_notes_preamble",
            "2",
            4,
        ),
    ];

    for (label, sliced, source, source_kind, skip_rows, expected_columns) in cases {
        let assert = profile_cmd()
            .arg("emit-discovery")
            .arg(fixture_path(sliced))
            .arg("--source-file")
            .arg(fixture_path(source))
            .arg("--skip-rows")
            .arg(skip_rows)
            .arg("--source-kind")
            .arg(source_kind)
            .arg("--json")
            .arg("--no-witness")
            .assert();
        let envelope = parse_stdout_json(&assert);
        common::assert_success_exit!(assert);

        assert_eq!(
            envelope["result"]["version"], "profile.discovery.v0",
            "{label}"
        );
        assert_eq!(
            envelope["result"]["candidate_template"]["source_kind"], source_kind,
            "{label}"
        );
        assert_eq!(
            envelope["result"]["candidate_template"]["column_count"], expected_columns,
            "{label}"
        );
    }
}

#[test]
fn emit_discovery_refuses_inconsistent_sliced_csv() {
    let assert = profile_cmd()
        .arg("emit-discovery")
        .arg(fixture_path("discovery/bad_sliced_inconsistent.csv"))
        .arg("--source-file")
        .arg(fixture_path("discovery/generic_source.csv"))
        .arg("--skip-rows")
        .arg("2")
        .arg("--json")
        .arg("--no-witness")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

    assert_eq!(envelope["outcome"], "REFUSAL");
    assert_eq!(envelope["result"]["code"], "E_CSV_PARSE");
    assert!(
        envelope["result"]["detail"]["error"]
            .as_str()
            .is_some_and(|error| error.contains("E_BAD_SLICE"))
    );
}

#[test]
fn emit_discovery_refuses_binary_source_file() {
    let workspace = temp_workspace();
    let binary_path = workspace.path().join("source.bin");
    fs::write(&binary_path, [0xff, 0x00, 0xfe]).expect("write binary source");

    let assert = profile_cmd()
        .arg("emit-discovery")
        .arg(fixture_path("discovery/generic_sliced.csv"))
        .arg("--source-file")
        .arg(&binary_path)
        .arg("--skip-rows")
        .arg("2")
        .arg("--json")
        .arg("--no-witness")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

    assert_eq!(envelope["outcome"], "REFUSAL");
    assert_eq!(envelope["result"]["code"], "E_CSV_PARSE");
    assert!(
        envelope["result"]["detail"]["error"]
            .as_str()
            .is_some_and(|error| error.contains("E_NOT_TEXT"))
    );
}
