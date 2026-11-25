//! bd-6aj: raw CSV rows never reach stdout by default. `profile slice` needs `--out <path>` or
//! `--explicit`; human mode with neither refuses (E_OUTPUT_REQUIRED, exit 2) with a shell-quoted
//! next_command. JSON mode without --out prints metadata only (rows only with --explicit).

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::{fixture_path, temp_workspace};
use serde_json::Value;

/// Values from tests/fixtures/slice/multi_header.csv's data rows.
const CANARIES: [&str; 4] = ["Alice Example", "Bob Example", "1001", "44.00"];
const SLICED_CSV: &str = "Portfolio.ID,Portfolio.Name,Metric.Amount,Metric.Rate\n1001,Alice Example,12.50,0.40\n1002,Bob Example,44.00,0.35\n";
const SLICE_FLAGS: [&str; 8] = [
    "--mode",
    "multi_row_header",
    "--header-rows",
    "1,2",
    "--data-starts-at",
    "3",
    "--header-merge",
    "ffill_concat",
];

fn slice(input: &Path, extra: &[&str], cwd: &Path, ledger: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_profile"));
    command
        .current_dir(cwd)
        .env_remove("PROFILE_REDACTION_KEY")
        .arg("slice")
        .arg(input)
        .args(SLICE_FLAGS)
        .args(extra);
    match ledger {
        Some(ledger) => command.env("EPISTEMIC_WITNESS", ledger),
        None => command.arg("--no-witness"),
    };
    command.output().expect("run profile slice")
}

fn printed(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_no_canaries(text: &str, what: &str) {
    for canary in CANARIES {
        assert!(
            !text.contains(canary),
            "{what}: {canary} reached output: {text}"
        );
    }
}

fn input() -> PathBuf {
    fixture_path("slice/multi_header.csv")
}

#[test]
fn human_mode_without_out_or_explicit_refuses_and_prints_no_row() {
    let dir = temp_workspace();
    let ledger = dir.path().join("ledger.jsonl");
    let manifest = dir.path().join("m.json");
    let output = slice(
        &input(),
        &["--emit-manifest", manifest.to_str().unwrap()],
        dir.path(),
        Some(&ledger),
    );
    assert_eq!(output.status.code(), Some(2), "{}", printed(&output));
    let text = printed(&output);
    assert_no_canaries(&text, "refusal");
    assert!(text.contains("E_OUTPUT_REQUIRED"), "{text}");
    assert!(
        text.contains("--out") && text.contains("--explicit"),
        "{text}"
    );
    // The refusal happens before any side effect: no manifest, no witness record.
    assert!(!manifest.exists());
    assert!(!ledger.exists() || fs::read_to_string(&ledger).unwrap().is_empty());
}

#[test]
fn the_refusal_next_command_is_shell_quoted_and_works() {
    let dir = temp_workspace();
    let awkward = dir.path().join("data (1) it's.csv");
    fs::copy(input(), &awkward).unwrap();
    let refusal = slice(&awkward, &[], dir.path(), None);
    assert_eq!(refusal.status.code(), Some(2));
    let text = printed(&refusal);
    assert_no_canaries(&text, "refusal");
    let next = text
        .lines()
        .find_map(|line| line.strip_prefix("Suggested action: "))
        .unwrap_or_else(|| panic!("no suggested action in: {text}"))
        .to_owned();
    // The path with spaces, parentheses and an apostrophe is single-quoted (apostrophe escaped).
    assert!(
        next.contains("'") && next.contains("data (1) it'\\''s.csv"),
        "{next}"
    );
    assert!(
        next.contains("--out") && next.contains("--explicit"),
        "{next}"
    );
    // Run it through a shell, adding the slice flags before the trailing comment.
    let (runnable, _) = next.split_once("   #").unwrap_or((next.as_str(), ""));
    let rest = runnable
        .strip_prefix("profile ")
        .expect("starts with profile");
    let shell = format!(
        "'{}' --no-witness {rest} {}",
        env!("CARGO_BIN_EXE_profile"),
        SLICE_FLAGS.join(" ")
    );
    let rerun = Command::new("sh")
        .arg("-c")
        .arg(shell)
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert_eq!(rerun.status.code(), Some(0), "{}", printed(&rerun));
    assert_no_canaries(&printed(&rerun), "next_command rerun");
    assert_eq!(
        fs::read_to_string(dir.path().join("sliced.csv")).unwrap(),
        SLICED_CSV
    );
}

#[test]
fn out_writes_the_file_and_stdout_carries_only_the_summary() {
    let dir = temp_workspace();
    for (label, flags) in [
        ("human", vec![]),
        ("json", vec!["--json"]),
        // --explicit never puts the CSV on stdout when --out is given.
        ("human+explicit", vec!["--explicit"]),
        ("json+explicit", vec!["--json", "--explicit"]),
    ] {
        let out = dir.path().join(format!("{label}.csv"));
        let mut args = flags.clone();
        args.extend(["--out", out.to_str().unwrap()]);
        let output = slice(&input(), &args, dir.path(), None);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{label}: {}",
            printed(&output)
        );
        assert_no_canaries(&printed(&output), label);
        assert_eq!(fs::read_to_string(&out).unwrap(), SLICED_CSV, "{label}");
    }
}

#[test]
fn explicit_without_out_streams_the_csv_to_stdout() {
    let dir = temp_workspace();
    let output = slice(&input(), &["--explicit"], dir.path(), None);
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    assert_eq!(String::from_utf8_lossy(&output.stdout), SLICED_CSV);
}

#[test]
fn json_mode_without_out_never_puts_rows_on_stdout_unless_explicit() {
    let dir = temp_workspace();
    let default = slice(&input(), &["--json"], dir.path(), None);
    assert_eq!(default.status.code(), Some(0));
    assert_no_canaries(&printed(&default), "json default");
    let envelope: Value = serde_json::from_slice(&default.stdout).unwrap();
    assert!(envelope["result"].get("slice_csv").is_none());
    assert_eq!(envelope["result"]["rows"]["output_data_rows"], 2);
    // With --explicit and no --out the rows are in the envelope's slice_csv.
    let explicit = slice(&input(), &["--json", "--explicit"], dir.path(), None);
    let envelope: Value = serde_json::from_slice(&explicit.stdout).unwrap();
    assert_eq!(envelope["result"]["slice_csv"], SLICED_CSV);
}
