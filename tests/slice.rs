mod common;

use common::{fixture_path, parse_stdout_json, profile_cmd, temp_workspace};
use serde_json::{Value, json};
use std::fs;
use std::io::{BufWriter, Write};
use std::time::{Duration, Instant};

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

#[test]
fn slice_profile_override_records_warning_in_json() {
    let assert = profile_cmd()
        .arg("slice")
        .arg(fixture_path("slice/preamble.csv"))
        .arg("--profile-path")
        .arg(fixture_path("slice/preparse_profile.yaml"))
        .arg("--skip-rows")
        .arg("2")
        .arg("--json")
        .arg("--no-witness")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert!(
        envelope["result"]["warnings"]
            .as_array()
            .is_some_and(|warnings| !warnings.is_empty())
    );
    assert!(
        envelope["result"]["warnings"][0]
            .as_str()
            .is_some_and(|warning| warning.contains("--skip-rows"))
    );
}

#[test]
fn slice_profile_override_emits_human_warning_to_stderr() {
    let workspace = temp_workspace();
    let assert = profile_cmd()
        .arg("slice")
        .arg(fixture_path("slice/preamble.csv"))
        .arg("--profile-path")
        .arg(fixture_path("slice/preparse_profile.yaml"))
        .arg("--skip-rows")
        .arg("2")
        .arg("--out")
        .arg(workspace.path().join("clean.csv"))
        .arg("--no-witness")
        .assert();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    common::assert_success_exit!(assert);

    assert!(stderr.contains("Warning: profile pre_parse directives were overridden"));
}

#[test]
fn slice_warns_when_expected_modal_column_count_mismatches_output() {
    let assert = profile_cmd()
        .arg("slice")
        .arg(fixture_path("slice/modal_mismatch.csv"))
        .arg("--profile-path")
        .arg(fixture_path("slice/modal_mismatch_profile.yaml"))
        .arg("--json")
        .arg("--no-witness")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert!(
        envelope["result"]["warnings"]
            .as_array()
            .is_some_and(|warnings| {
                warnings.iter().any(|warning| {
                    warning
                        .as_str()
                        .is_some_and(|text| text.contains("expected_shape.modal_column_count"))
                })
            })
    );
}

#[test]
fn slice_refuses_when_data_rows_are_absent_after_directives() {
    let assert = profile_cmd()
        .arg("slice")
        .arg(fixture_path("slice/edge_only_preamble.csv"))
        .arg("--mode")
        .arg("preamble_skip")
        .arg("--skip-rows")
        .arg("1")
        .arg("--header-at-row")
        .arg("2")
        .arg("--json")
        .arg("--no-witness")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

    assert_eq!(envelope["result"]["code"], "E_EMPTY");
    assert_eq!(
        envelope["result"]["detail"]["reason"],
        "no data rows after slice directives applied"
    );
}

#[test]
fn slice_output_hash_is_reproducible_across_runs() {
    let mut hashes = Vec::new();
    for _ in 0..5 {
        let assert = profile_cmd()
            .arg("slice")
            .arg(fixture_path("slice/preamble.csv"))
            .arg("--profile-path")
            .arg(fixture_path("slice/preparse_profile.yaml"))
            .arg("--json")
            .arg("--no-witness")
            .assert();
        let envelope = parse_stdout_json(&assert);
        common::assert_success_exit!(assert);
        hashes.push(
            envelope["result"]["output_hash"]
                .as_str()
                .expect("output hash")
                .to_owned(),
        );
    }

    assert!(hashes.windows(2).all(|window| window[0] == window[1]));
}

#[test]
fn slice_fixture_profiles_round_trip_with_lint() {
    let workspace = temp_workspace();
    let cases = [
        ("slice/preamble.csv", "slice/preparse_profile.yaml"),
        ("slice/multi_header.csv", "slice/multi_header_profile.yaml"),
        ("slice/units.csv", "slice/units_profile.yaml"),
        ("slice/clean_flat.csv", "slice/clean_flat_profile.yaml"),
    ];

    for (index, (input, profile)) in cases.iter().enumerate() {
        let sliced_output = workspace
            .path()
            .join(format!("slice_roundtrip_{index}.csv"));
        let slice_assert = profile_cmd()
            .arg("slice")
            .arg(fixture_path(input))
            .arg("--profile-path")
            .arg(fixture_path(profile))
            .arg("--out")
            .arg(&sliced_output)
            .arg("--json")
            .arg("--no-witness")
            .assert();
        common::assert_success_exit!(slice_assert);

        let lint_assert = profile_cmd()
            .arg("lint")
            .arg(fixture_path(profile))
            .arg("--against")
            .arg(&sliced_output)
            .arg("--json")
            .arg("--no-witness")
            .assert();
        let lint_envelope = parse_stdout_json(&lint_assert);
        common::assert_success_exit!(lint_assert);
        assert!(
            lint_envelope["result"]["issues"]
                .as_array()
                .is_some_and(|issues| issues.is_empty())
        );
    }
}

#[test]
#[ignore = "performance smoke; run manually in release workflows"]
fn slice_100mb_dataset_perf_smoke() {
    let workspace = temp_workspace();
    let large_csv = workspace.path().join("large_100mb.csv");
    let out = workspace.path().join("large_100mb.clean.csv");

    let file = fs::File::create(&large_csv).expect("create large fixture");
    let mut writer = BufWriter::new(file);
    writeln!(writer, "id,value").expect("write header");
    let mut bytes_written = "id,value\n".len() as u64;
    let row = b"1234567890,abcdefghijklmnopqrstuvwxyz0123456789\n";
    while bytes_written < 100 * 1024 * 1024 {
        writer.write_all(row).expect("write row");
        bytes_written += row.len() as u64;
    }
    writer.flush().expect("flush fixture");

    let start = Instant::now();
    let assert = profile_cmd()
        .arg("slice")
        .arg(&large_csv)
        .arg("--mode")
        .arg("preamble_skip")
        .arg("--skip-rows")
        .arg("0")
        .arg("--header-at-row")
        .arg("1")
        .arg("--data-starts-at")
        .arg("2")
        .arg("--out")
        .arg(&out)
        .arg("--json")
        .arg("--no-witness")
        .assert();
    common::assert_success_exit!(assert);
    assert!(start.elapsed() < Duration::from_secs(15));
}
