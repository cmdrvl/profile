//! bd-ds5: dataset header text (a banner, a methodology paragraph or a mis-sliced data row on a
//! messy sheet) is never printed without --explicit. Default = ordinal refs in data order; opt-in
//! keyed HMAC via PROFILE_REDACTION_KEY; an unusable key refuses; the key never leaks.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::{fixture_path, temp_workspace};
use serde_json::{Value, json};

const KEY: &str = "0123456789abcdef";
const OTHER_KEY: &str = "fedcba9876543210";
const KEY_ENV: &str = "PROFILE_REDACTION_KEY";

/// Planted in tests/fixtures/slice/make_preamble_xlsx.py: banner, note and two data rows.
const CANARIES: [&str; 4] = [
    "CANARY-BANNER-7f3a1c",
    "CANARY-NOTE-2b9c4e",
    "CANARY-DATA5-5d1e8a",
    "CANARY-DATA6-9c0b3f",
];

fn preamble_xlsx() -> PathBuf {
    fixture_path("slice/preamble_canary.xlsx")
}

/// `profile --json <args>` with an optional key and an optional witness ledger.
fn profile(args: &[&str], key: Option<&str>, witness: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_profile"));
    command.arg("--json").env_remove(KEY_ENV);
    if let Some(key) = key {
        command.env(KEY_ENV, key);
    }
    match witness {
        Some(ledger) => command.env("EPISTEMIC_WITNESS", ledger),
        None => command.arg("--no-witness"),
    };
    command.args(args).output().expect("run profile")
}

fn json_of(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout was not JSON ({error}): {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn text(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn xlsx_arg(path: &Path) -> String {
    path.display().to_string()
}

/// Every command that prints dataset header text, each run against the mis-sliced workbook.
fn header_commands(xlsx: &str) -> Vec<(&'static str, Vec<String>)> {
    let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
    vec![
        // The banner row is the header.
        (
            "slice-banner",
            s(&["slice", xlsx, "--sheet", "Report", "--header-at-row", "1"]),
        ),
        // A data row is the header (skip-rows lands one row too far).
        (
            "slice-data-row",
            s(&["slice", xlsx, "--sheet", "Report", "--header-at-row", "5"]),
        ),
        // inspect / stats / suggest-key take the first row of the sheet as the header: the banner.
        ("inspect", s(&["inspect", xlsx, "--sheet", "Report"])),
        ("stats", s(&["stats", xlsx, "--sheet", "Report"])),
        (
            "suggest-key",
            s(&["suggest-key", xlsx, "--sheet", "Report"]),
        ),
    ]
}

#[test]
fn canaries_from_a_banner_row_and_a_mis_sliced_data_row_never_reach_stdout_stderr_or_the_ledger() {
    let workspace = temp_workspace();
    let xlsx = xlsx_arg(&preamble_xlsx());
    for (name, args) in header_commands(&xlsx) {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        for key in [None, Some(KEY)] {
            let ledger = workspace
                .path()
                .join(format!("{name}-{}.jsonl", key.is_some()));
            let output = profile(&args, key, Some(&ledger));
            assert_eq!(output.status.code(), Some(0), "{name}: {}", text(&output));
            let printed = text(&output);
            let recorded = fs::read_to_string(&ledger).unwrap_or_default();
            for canary in CANARIES {
                assert!(
                    !printed.contains(canary),
                    "{name} key={}: {canary} printed: {printed}",
                    key.is_some()
                );
                assert!(
                    !recorded.contains(canary),
                    "{name} key={}: {canary} in the ledger",
                    key.is_some()
                );
            }
        }
    }
}

#[test]
fn explicit_still_shows_the_full_header_text() {
    let xlsx = xlsx_arg(&preamble_xlsx());
    let banner = profile(
        &[
            "--explicit",
            "slice",
            &xlsx,
            "--sheet",
            "Report",
            "--header-at-row",
            "1",
        ],
        None,
        None,
    );
    assert_eq!(banner.status.code(), Some(0), "{}", text(&banner));
    assert!(text(&banner).contains("CANARY-BANNER-7f3a1c"));
    let data_row = profile(
        &[
            "--explicit",
            "slice",
            &xlsx,
            "--sheet",
            "Report",
            "--header-at-row",
            "5",
        ],
        None,
        None,
    );
    assert!(json_of(&data_row)["result"]["columns"][0] == "CANARY-DATA5-5d1e8a");
    for command in ["inspect", "stats"] {
        let output = profile(
            &["--explicit", command, &xlsx, "--sheet", "Report"],
            None,
            None,
        );
        assert!(
            json_of(&output)["result"]["columns"][0]["name"]
                .as_str()
                .is_some_and(|name| name.contains("CANARY-BANNER-7f3a1c")),
            "{command}"
        );
    }
    let key = profile(
        &["--explicit", "suggest-key", &xlsx, "--sheet", "Report"],
        None,
        None,
    );
    assert!(text(&key).contains("CANARY-BANNER-7f3a1c"));
}

#[test]
fn a_banner_merged_across_columns_is_one_ref_repeated_not_n_leaks() {
    let output = profile(
        &[
            "slice",
            &xlsx_arg(&preamble_xlsx()),
            "--sheet",
            "Report",
            "--header-at-row",
            "1",
        ],
        None,
        None,
    );
    assert_eq!(
        json_of(&output)["result"]["columns"],
        json!([
            "redacted:ref:c1",
            "redacted:ref:c1",
            "redacted:ref:c1",
            "redacted:ref:c1"
        ])
    );
}

fn write_csv(dir: &Path, name: &str, body: &str) -> String {
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    path.display().to_string()
}

/// (id, name) of each column of `stats`/`inspect`, from a default run and an --explicit run.
fn id_to_name(command: &str, file: &str) -> Vec<(String, String)> {
    let default = json_of(&profile(&[command, file], None, None));
    let explicit = json_of(&profile(&["--explicit", command, file], None, None));
    default["result"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .zip(explicit["result"]["columns"].as_array().unwrap())
        .map(|(d, e)| {
            (
                d["id"].as_str().unwrap().to_owned(),
                e["name"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn ordinal_refs_follow_data_order_not_name_order() {
    let dir = temp_workspace();
    let zebra_first = write_csv(dir.path(), "a.csv", "zebra,apple,mango\n1,x,3\n2,y,4\n");
    let apple_first = write_csv(dir.path(), "b.csv", "apple,mango,zebra\nx,3,1\ny,4,2\n");
    for command in ["stats", "inspect"] {
        assert_eq!(
            id_to_name(command, &zebra_first),
            [
                ("redacted:ref:c1".into(), "zebra".into()),
                ("redacted:ref:c2".into(), "apple".into()),
                ("redacted:ref:c3".into(), "mango".into()),
            ],
            "{command}"
        );
        // A name-sorted numbering would call apple c1 in both files.
        assert_eq!(
            id_to_name(command, &apple_first),
            [
                ("redacted:ref:c1".into(), "apple".into()),
                ("redacted:ref:c2".into(), "mango".into()),
                ("redacted:ref:c3".into(), "zebra".into()),
            ],
            "{command}"
        );
    }
    // slice and suggest-key use the same numbering.
    let sliced = json_of(&profile(&["slice", &zebra_first], None, None));
    assert_eq!(
        sliced["result"]["columns"],
        json!(["redacted:ref:c1", "redacted:ref:c2", "redacted:ref:c3"])
    );
    let keys = json_of(&profile(
        &["suggest-key", &zebra_first, "--top", "3"],
        None,
        None,
    ));
    let named = json_of(&profile(
        &["--explicit", "suggest-key", &zebra_first, "--top", "3"],
        None,
        None,
    ));
    let order = |v: &Value| -> Vec<Value> {
        v["result"]["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["position"].clone())
            .collect()
    };
    assert_eq!(
        order(&keys),
        order(&named),
        "ranking does not depend on whether names are shown"
    );
    // Equal header text in one file shares a ref.
    let dup = write_csv(dir.path(), "dup.csv", "Total,Item,Total\n1,a,2\n");
    let dup_ids = json_of(&profile(&["slice", &dup], None, None));
    assert_eq!(
        dup_ids["result"]["columns"],
        json!(["redacted:ref:c1", "redacted:ref:c2", "redacted:ref:c1"])
    );
}

#[test]
fn output_is_byte_identical_across_runs_default_and_keyed() {
    let xlsx = xlsx_arg(&preamble_xlsx());
    for (name, args) in header_commands(&xlsx) {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        for key in [None, Some(KEY)] {
            let one = profile(&args, key, None);
            let two = profile(&args, key, None);
            assert_eq!(one.stdout, two.stdout, "{name} key={}", key.is_some());
        }
    }
}

/// Every `redacted:hmac-sha256:<32 hex>` marker in `text`.
fn hmac_markers(text: &str) -> Vec<String> {
    const PREFIX: &str = "redacted:hmac-sha256:";
    text.match_indices(PREFIX)
        .map(|(at, _)| text[at..at + PREFIX.len() + 32].to_owned())
        .collect()
}

#[test]
fn keyed_markers_match_an_openssl_known_answer_and_correlate_across_files() {
    let dir = temp_workspace();
    // printf 'column\0loan_id' | openssl dgst -sha256 -hmac 0123456789abcdef | cut -c1-32
    let known = "redacted:hmac-sha256:9f6181c104894742edc83d76f18dc14f";
    let one = write_csv(dir.path(), "one.csv", "loan_id,region\nL1,East\nL2,West\n");
    let two = write_csv(dir.path(), "two.csv", "balance,loan_id\n1,L9\n2,L8\n");
    let first = json_of(&profile(&["stats", &one], Some(KEY), None));
    assert_eq!(first["result"]["redaction"], "keyed");
    assert_eq!(first["result"]["columns"][0]["id"], known);
    // The same key gives the same marker for the same header in another file, in another position.
    let second = json_of(&profile(&["inspect", &two], Some(KEY), None));
    assert_eq!(second["result"]["columns"][1]["id"], known);
    for command in ["slice", "suggest-key"] {
        let out = text(&profile(&[command, &one], Some(KEY), None));
        assert!(out.contains(known), "{command}: {out}");
    }
    // A different key changes every marker; no marker is shared.
    for command in ["stats", "inspect", "slice", "suggest-key"] {
        let a = hmac_markers(&text(&profile(&[command, &one], Some(KEY), None)));
        let b = hmac_markers(&text(&profile(&[command, &one], Some(OTHER_KEY), None)));
        assert!(!a.is_empty() && a.len() == b.len(), "{command}");
        assert!(a.iter().zip(&b).all(|(x, y)| x != y), "{command}");
    }
}

#[test]
fn an_empty_or_short_key_refuses_with_a_typed_code_for_every_covered_command() {
    let dir = temp_workspace();
    let csv = write_csv(dir.path(), "a.csv", "loan_id,region\nL1,East\n");
    let ledger = dir.path().join("ledger.jsonl");
    for command in ["slice", "inspect", "stats", "suggest-key"] {
        for (bad, reason) in [
            ("", "empty"),
            ("SHORT-KEY-xyz", "too_short"),
            ("0123456789abcde", "too_short"),
        ] {
            for explicit in [false, true] {
                let mut args = vec![command, csv.as_str()];
                if explicit {
                    args.insert(0, "--explicit");
                }
                let output = profile(&args, Some(bad), Some(&ledger));
                assert_eq!(
                    output.status.code(),
                    Some(2),
                    "{command} {bad:?} explicit={explicit}"
                );
                let envelope = json_of(&output);
                assert_eq!(envelope["outcome"], "REFUSAL");
                assert_eq!(envelope["result"]["code"], "E_REDACTION_KEY_INVALID");
                assert_eq!(
                    envelope["result"]["detail"],
                    json!({"env": KEY_ENV, "reason": reason, "min_bytes": 16})
                );
                assert!(
                    bad.is_empty() || !text(&output).contains(bad),
                    "{command}: key echoed"
                );
            }
        }
    }
    assert!(
        !ledger.exists() || fs::read_to_string(&ledger).unwrap().is_empty(),
        "a refusal writes no witness record"
    );
    // Commands that print no dataset text are not affected by the variable.
    let list = profile(&["list"], Some("SHORT"), None);
    assert_ne!(json_of(&list)["result"]["code"], "E_REDACTION_KEY_INVALID");
}

#[test]
fn the_key_never_appears_in_stdout_stderr_the_ledger_or_doctor() {
    let dir = temp_workspace();
    let secret = "SECRET-KEY-MATERIAL-9f3a1c";
    let csv = write_csv(dir.path(), "a.csv", "loan_id,region\nL1,East\n");
    for command in ["slice", "inspect", "stats", "suggest-key"] {
        let ledger = dir.path().join(format!("{command}.jsonl"));
        let output = profile(&[command, &csv], Some(secret), Some(&ledger));
        assert_eq!(output.status.code(), Some(0), "{command}");
        assert!(!text(&output).contains(secret), "{command}");
        // `inspect` writes no witness record; the others do.
        let recorded = fs::read_to_string(&ledger).unwrap_or_default();
        assert!(!recorded.contains(secret), "{command}: key in the ledger");
        assert!(
            command == "inspect" || !recorded.is_empty(),
            "{command}: a ledger record was written"
        );
    }
    for (key, severity, wording, code) in [
        (Some(secret), "info", "configured", 0),
        (None, "info", "not configured", 0),
        (Some("tiny"), "error", "unusable", 1),
    ] {
        let output = profile(&["doctor", "health"], key, None);
        let printed = text(&output);
        if let Some(key) = key {
            assert!(!printed.contains(key), "doctor leaked the key: {printed}");
        }
        let report = json_of(&output);
        let finding = report["findings"]
            .as_array()
            .or_else(|| report["result"]["findings"].as_array())
            .and_then(|findings| findings.iter().find(|f| f["id"] == "profile.redaction_key"))
            .unwrap_or_else(|| panic!("no profile.redaction_key finding: {printed}"));
        assert_eq!(finding["severity"], severity);
        assert!(
            finding["message"].as_str().unwrap().contains(wording),
            "{finding}"
        );
        assert_eq!(output.status.code(), Some(code), "{key:?}");
    }
}

#[test]
fn no_unsalted_hash_of_a_header_appears_in_any_output() {
    let dir = temp_workspace();
    let csv = write_csv(dir.path(), "a.csv", "loan_id,region,balance\nL1,East,1\n");
    let mut dictionary = Vec::new();
    for name in ["loan_id", "region", "balance"] {
        let hex = blake3::hash(name.as_bytes()).to_hex().to_string();
        dictionary.push(hex[..16].to_owned());
        dictionary.push(hex);
    }
    // blake3(loan_id) is d3411460f2fd7fe9...: the dictionary matcher is not vacuous.
    assert!(dictionary.contains(&"d3411460f2fd7fe9".to_owned()));
    for command in ["slice", "inspect", "stats", "suggest-key"] {
        for key in [None, Some(KEY)] {
            let printed = text(&profile(&[command, &csv], key, None));
            for token in &dictionary {
                assert!(
                    !printed.contains(token.as_str()),
                    "{command} key={}: {token}",
                    key.is_some()
                );
            }
        }
    }
}

#[test]
fn refusals_withhold_dataset_header_and_sheet_text_by_default() {
    let xlsx = fixture_path("formats/new_two_sheets.xlsx")
        .display()
        .to_string();
    let default = profile(&["stats", &xlsx], None, None);
    let context = json_of(&default)["result"]["detail"]["context"].clone();
    assert_eq!(context, json!({"sheet_count": 2}));
    assert!(!text(&default).contains("loans"));
    let explicit = profile(&["--explicit", "stats", &xlsx], None, None);
    assert!(text(&explicit).contains("loans"));
}

/// Run a `next_command` string through `sh`, with `profile` standing for the binary under test.
fn run_next_command(next: &str, cwd: &Path) -> Output {
    let rest = next
        .strip_prefix("profile ")
        .expect("starts with the real binary name");
    Command::new("sh")
        .arg("-c")
        .arg(format!("'{}' {rest}", env!("CARGO_BIN_EXE_profile")))
        .current_dir(cwd)
        .env_remove(KEY_ENV)
        .output()
        .expect("sh")
}

#[test]
fn ambiguous_sheet_next_command_names_the_real_subcommand_and_quotes_awkward_paths() {
    let dir = temp_workspace();
    for file_name in ["report (1).xlsx", "it's a $report; (2).xlsx", "plain.xlsx"] {
        let path = dir.path().join(file_name);
        fs::copy(fixture_path("formats/new_two_sheets.xlsx"), &path).unwrap();
        let shown = path.display().to_string();
        for command in ["stats", "inspect", "slice", "suggest-key"] {
            let refusal = json_of(&profile(&[command, &shown, "--no-witness"], None, None));
            assert_eq!(
                refusal["result"]["detail"]["tabular_code"],
                "E_AMBIGUOUS_SHEET"
            );
            let next = refusal["result"]["next_command"]
                .as_str()
                .unwrap()
                .to_owned();
            assert!(
                !next.contains("<command>") && !next.contains("<name|index>"),
                "{next}"
            );
            assert!(
                next.starts_with(&format!("profile {command} ")),
                "{command}: {next}"
            );
            // The quoted path round-trips: running the command in a shell succeeds.
            // Sheet 0 of this workbook has no data rows, so rerun on sheet 1: the point is that
            // the quoted path survives the shell and the real subcommand parses.
            let rerun = run_next_command(
                &format!(
                    "{} --json --no-witness",
                    next.replace("--sheet 0", "--sheet 1")
                ),
                dir.path(),
            );
            assert_eq!(
                rerun.status.code(),
                Some(0),
                "{command} {file_name}: {next}\n{}",
                text(&rerun)
            );
        }
    }
    // The same for E_SHEET_NOT_FOUND and for the --against commands.
    let path = dir.path().join("report (1).xlsx");
    let shown = path.display().to_string();
    let missing = json_of(&profile(&["stats", &shown, "--sheet", "nope"], None, None));
    assert_eq!(
        missing["result"]["detail"]["tabular_code"],
        "E_SHEET_NOT_FOUND"
    );
    assert!(
        missing["result"]["next_command"]
            .as_str()
            .unwrap()
            .contains("'")
    );
    let profile_path = fixture_path("profiles/valid/frozen_complete.yaml")
        .display()
        .to_string();
    let lint = json_of(&profile(
        &["lint", &profile_path, "--against", &shown],
        None,
        None,
    ));
    let next = lint["result"]["next_command"].as_str().unwrap();
    assert!(next.starts_with("profile lint "), "{next}");
    assert!(
        next.contains("--against '") && next.contains("report (1).xlsx'"),
        "{next}"
    );
}
