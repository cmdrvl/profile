//! TSV, JSONL, Parquet and XLSX datasets through the shared tabular reader. The same loan
//! dataset exists as CSV, TSV, JSONL, Parquet and XLSX (independent writers, see
//! tests/fixtures/formats/make_format_fixtures.py). `draft init`, `suggest-key`, `lint`,
//! `stats` and `slice` must report exactly what they report for the CSV.

mod common;

use std::fs;

use common::{fixture_path, parse_stdout_json, profile_cmd, temp_workspace};
use serde_json::{Value, json};

const EXTS: [&str; 5] = ["csv", "tsv", "jsonl", "parquet", "xlsx"];

fn fx(name: &str) -> String {
    fixture_path(&format!("formats/{name}"))
        .display()
        .to_string()
}

/// Run `profile --json --no-witness <args>`; returns (exit code, stdout JSON).
fn run(args: &[&str]) -> (Option<i32>, Value) {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .args(args)
        .assert();
    let code = assert.get_output().status.code();
    (code, parse_stdout_json(&assert))
}

#[test]
fn stats_is_identical_across_containers() {
    let (code, baseline) = run(&["stats", &fx("old.csv")]);
    assert_eq!(code, Some(0));
    assert_eq!(baseline["result"]["row_count"], 3);
    assert_eq!(baseline["result"]["column_count"], 4);
    for ext in EXTS {
        let (code, got) = run(&["stats", &fx(&format!("old.{ext}"))]);
        assert_eq!(code, Some(0), "old.{ext}");
        assert_eq!(got["result"], baseline["result"], "old.{ext}");
    }
}

#[test]
fn suggest_key_is_identical_across_containers() {
    let (_, baseline) = run(&["suggest-key", &fx("old.csv")]);
    assert_eq!(baseline["result"]["candidates"][0]["column"], "loan_id");
    for ext in EXTS {
        let (code, got) = run(&["suggest-key", &fx(&format!("old.{ext}"))]);
        assert_eq!(code, Some(0), "old.{ext}");
        assert_eq!(got["result"], baseline["result"], "old.{ext}");
    }
}

#[test]
fn draft_init_writes_the_same_profile_for_every_container() {
    let dir = temp_workspace();
    let init = |ext: &str| -> String {
        let out = dir.path().join(format!("{ext}.yaml"));
        let (code, _) = run(&[
            "draft",
            "init",
            &fx(&format!("old.{ext}")),
            "--out",
            out.to_str().unwrap(),
            "--key",
            "loan_id",
        ]);
        assert_eq!(code, Some(0), "draft init old.{ext}");
        fs::read_to_string(out).unwrap()
    };
    let baseline = init("csv");
    for ext in EXTS {
        assert_eq!(init(ext), baseline, "draft init old.{ext}");
    }
}

#[test]
fn lint_is_identical_across_containers_clean_and_with_issues() {
    let dir = temp_workspace();
    let profile = dir.path().join("p.yaml");
    let (code, _) = run(&[
        "draft",
        "init",
        &fx("old.csv"),
        "--out",
        profile.to_str().unwrap(),
        "--key",
        "loan_id",
    ]);
    assert_eq!(code, Some(0));
    let profile = profile.to_str().unwrap();

    for ext in EXTS {
        let (code, got) = run(&["lint", profile, "--against", &fx(&format!("new.{ext}"))]);
        assert_eq!(code, Some(0), "clean new.{ext}: {got}");
        assert_eq!(got["result"]["issues"], json!([]), "new.{ext}");
    }

    // Planted negative: a profile for a different column set must report issues, and the same
    // issues whatever the container.
    let other = dir.path().join("other.yaml");
    fs::write(
        &other,
        "schema_version: 1\nstatus: draft\nformat: csv\nequivalence:\n  float_decimals: 6\n  trim_strings: true\nkey:\n- loan_id\ninclude_columns:\n- loan_id\n- balance\n- term_months\n",
    )
    .unwrap();
    let other = other.to_str().unwrap();
    let (code, baseline) = run(&["lint", other, "--against", &fx("old.csv")]);
    assert_eq!(code, Some(1), "{baseline}");
    assert!(
        !baseline["result"]["issues"].as_array().unwrap().is_empty(),
        "{baseline}"
    );
    for ext in EXTS {
        let (code, got) = run(&["lint", other, "--against", &fx(&format!("old.{ext}"))]);
        assert_eq!(code, Some(1), "old.{ext}");
        assert_eq!(got["result"], baseline["result"], "old.{ext}");
    }
}

#[test]
fn slice_reports_the_same_shape_and_hashes_the_original_file() {
    let dir = temp_workspace();
    let (code, baseline) = run(&["slice", &fx("old.csv")]);
    assert_eq!(code, Some(0));
    let strip = |mut v: Value| -> Value {
        v["result"].as_object_mut().unwrap().remove("input_path");
        v
    };
    let baseline_result = strip(baseline)["result"].clone();
    for ext in EXTS {
        let path = fx(&format!("old.{ext}"));
        let (code, got) = run(&["slice", &path]);
        assert_eq!(code, Some(0), "old.{ext}");
        assert_eq!(strip(got)["result"], baseline_result, "old.{ext}");

        // The manifest's input_hash identifies the file on disk, not the in-memory CSV.
        let manifest = dir.path().join(format!("{ext}.json"));
        let (code, _) = run(&[
            "slice",
            &path,
            "--emit-manifest",
            manifest.to_str().unwrap(),
        ]);
        assert_eq!(code, Some(0), "old.{ext} manifest");
        let manifest: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        let expected = format!(
            "blake3:{}",
            blake3::hash(&fs::read(&path).unwrap()).to_hex()
        );
        assert_eq!(manifest["input_hash"], json!(expected), "old.{ext}");
    }
}

#[test]
fn slice_output_csv_is_identical_for_every_container() {
    let dir = temp_workspace();
    let sliced = |ext: &str| -> String {
        let out = dir.path().join(format!("{ext}.csv"));
        let (code, _) = run(&[
            "slice",
            &fx(&format!("old.{ext}")),
            "--out",
            out.to_str().unwrap(),
        ]);
        assert_eq!(code, Some(0), "slice old.{ext}");
        fs::read_to_string(out).unwrap()
    };
    let baseline = sliced("csv");
    for ext in EXTS {
        assert_eq!(sliced(ext), baseline, "slice old.{ext}");
    }
}

// ---- worksheets ------------------------------------------------------------

#[test]
fn multi_sheet_workbook_without_a_sheet_refuses_and_names_the_sheets() {
    for command in ["stats", "suggest-key"] {
        let (code, got) = run(&[command, &fx("new_two_sheets.xlsx")]);
        assert_eq!(code, Some(2), "{command}");
        let refusal = &got["result"];
        assert_eq!(refusal["code"], "E_INPUT_FORMAT", "{command}: {got}");
        assert_eq!(refusal["detail"]["tabular_code"], "E_AMBIGUOUS_SHEET");
        assert_eq!(
            refusal["detail"]["context"]["sheets"],
            json!(["notes", "loans"])
        );
    }
}

#[test]
fn sheet_flag_selects_by_name_and_index_for_every_command() {
    let (_, baseline) = run(&["stats", &fx("new.csv")]);
    let (_, baseline_key) = run(&["suggest-key", &fx("new.csv")]);
    for sheet in ["loans", "1"] {
        let (code, got) = run(&["stats", &fx("new_two_sheets.xlsx"), "--sheet", sheet]);
        assert_eq!(code, Some(0), "stats --sheet {sheet}: {got}");
        assert_eq!(got["result"], baseline["result"], "--sheet {sheet}");
        let (_, got) = run(&["suggest-key", &fx("new_two_sheets.xlsx"), "--sheet", sheet]);
        assert_eq!(got["result"], baseline_key["result"], "--sheet {sheet}");
    }
}

#[test]
fn unknown_sheet_refuses_with_the_listing() {
    let (code, got) = run(&["stats", &fx("new_two_sheets.xlsx"), "--sheet", "missing"]);
    assert_eq!(code, Some(2));
    assert_eq!(got["result"]["detail"]["tabular_code"], "E_SHEET_NOT_FOUND");
    assert_eq!(
        got["result"]["detail"]["context"]["sheets"],
        json!(["notes", "loans"])
    );
}

// ---- refusals --------------------------------------------------------------

#[test]
fn unsupported_parquet_value_is_refused_without_the_value() {
    let (code, got) = run(&["stats", &fx("binary.parquet")]);
    assert_eq!(code, Some(2));
    assert_eq!(got["result"]["code"], "E_INPUT_FORMAT");
    assert_eq!(
        got["result"]["detail"]["tabular_code"],
        "E_UNSUPPORTED_VALUE"
    );
    assert!(!got.to_string().contains("secret"), "{got}");
}

#[test]
fn extension_that_disagrees_with_the_content_is_refused() {
    let (code, got) = run(&["suggest-key", &fx("not_really.parquet")]);
    assert_eq!(code, Some(2));
    assert_eq!(got["result"]["detail"]["tabular_code"], "E_FORMAT");
}

#[test]
fn missing_non_csv_file_is_the_ordinary_io_refusal() {
    let (code, got) = run(&["stats", &fx("does_not_exist.parquet")]);
    assert_eq!(code, Some(2));
    assert_eq!(got["result"]["code"], "E_IO");
}
