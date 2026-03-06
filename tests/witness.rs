mod common;

use std::fs;

use common::{assert_json_envelope_shape, parse_stdout_json, profile_cmd, temp_workspace};
use serde_json::json;

#[test]
fn witness_append_policy_respects_subcommand_and_no_witness_flag() {
    let workspace = temp_workspace();
    let home = workspace.path();
    let dataset = workspace.path().join("dataset.csv");
    fs::write(&dataset, "loan_id,balance\nL1,100\nL2,200\n").expect("dataset should be written");

    let stats_assert = profile_cmd()
        .env("HOME", home)
        .arg("stats")
        .arg(&dataset)
        .assert();
    common::assert_success_exit!(stats_assert);
    assert_eq!(ledger_line_count(home), 1);

    let list_assert = profile_cmd().env("HOME", home).arg("list").assert();
    common::assert_success_exit!(list_assert);
    assert_eq!(ledger_line_count(home), 1);

    let no_witness_assert = profile_cmd()
        .env("HOME", home)
        .arg("--no-witness")
        .arg("stats")
        .arg(&dataset)
        .assert();
    common::assert_success_exit!(no_witness_assert);
    assert_eq!(ledger_line_count(home), 1);
}

#[test]
fn witness_append_and_query_honor_epistemic_witness_override() {
    let workspace = temp_workspace();
    let home = workspace.path();
    let witness_root = temp_workspace();
    let ledger_path = witness_root.path().join("nested").join("witness.jsonl");
    let dataset = workspace.path().join("dataset.csv");
    fs::write(&dataset, "loan_id,balance\nL1,100\nL2,200\n").expect("dataset should be written");

    let stats_assert = profile_cmd()
        .env("HOME", home)
        .env("EPISTEMIC_WITNESS", &ledger_path)
        .arg("stats")
        .arg(&dataset)
        .assert();
    common::assert_success_exit!(stats_assert);
    assert_eq!(ledger_line_count_at(&ledger_path), 1);
    assert!(!home.join(".epistemic").join("witness.jsonl").exists());
    let record = read_ledger_record(&ledger_path, 0);
    assert_eq!(
        record.get("version").and_then(|value| value.as_str()),
        Some(env!("CARGO_PKG_VERSION"))
    );
    assert!(
        record
            .get("binary_hash")
            .and_then(|value| value.as_str())
            .is_some_and(|value| value.starts_with("blake3:"))
    );

    let count_assert = profile_cmd()
        .env("HOME", home)
        .env("EPISTEMIC_WITNESS", &ledger_path)
        .arg("--json")
        .arg("witness")
        .arg("count")
        .assert();
    let count_envelope = parse_stdout_json(&count_assert);
    common::assert_success_exit!(count_assert);
    assert_json_envelope_shape(&count_envelope);
    assert_eq!(
        count_envelope
            .get("result")
            .and_then(|r| r.get("count"))
            .and_then(|v| v.as_u64()),
        Some(1)
    );
}

#[test]
fn witness_query_last_count_read_ledger_deterministically() {
    let workspace = temp_workspace();
    let home = workspace.path();
    let ledger_path = home.join(".epistemic").join("witness.jsonl");
    fs::create_dir_all(ledger_path.parent().expect("ledger parent"))
        .expect("ledger directory should exist");

    let first = json!({
        "id": "blake3:first",
        "ts": "2026-02-27T00:00:00Z",
        "tool": "profile",
        "version": "0.0.0-test",
        "binary_hash": "blake3:binary-first",
        "inputs": [],
        "params": { "subcommand": "stats" },
        "output_hash": "blake3:a",
        "outcome": "SUCCESS",
        "exit_code": 0,
        "prev": null
    });
    let second = json!({
        "id": "blake3:second",
        "ts": "2026-02-27T00:01:00Z",
        "tool": "profile",
        "version": "0.0.0-test",
        "binary_hash": "blake3:binary-second",
        "inputs": [],
        "params": { "subcommand": "lint" },
        "output_hash": "blake3:b",
        "outcome": "ISSUES_FOUND",
        "exit_code": 1,
        "prev": "blake3:first"
    });
    let other_tool = json!({
        "id": "blake3:other",
        "ts": "2026-02-27T00:02:00Z",
        "tool": "lock",
        "version": "0.0.0-test",
        "binary_hash": "blake3:binary-other",
        "inputs": [],
        "params": {},
        "output_hash": "blake3:c",
        "outcome": "LOCK_CREATED",
        "exit_code": 0,
        "prev": "blake3:second"
    });
    let mut contents = serde_json::to_string(&first).expect("first JSON");
    contents.push('\n');
    contents.push_str(&serde_json::to_string(&second).expect("second JSON"));
    contents.push('\n');
    contents.push_str(&serde_json::to_string(&other_tool).expect("other JSON"));
    contents.push('\n');
    fs::write(&ledger_path, contents).expect("ledger should be written");

    let count_assert = profile_cmd()
        .env("HOME", home)
        .arg("--json")
        .arg("witness")
        .arg("count")
        .assert();
    let count_envelope = parse_stdout_json(&count_assert);
    common::assert_success_exit!(count_assert);
    assert_json_envelope_shape(&count_envelope);
    assert_eq!(
        count_envelope
            .get("result")
            .and_then(|r| r.get("count"))
            .and_then(|v| v.as_u64()),
        Some(2),
        "profile witness count should ignore other tools"
    );

    let last_assert = profile_cmd()
        .env("HOME", home)
        .arg("--json")
        .arg("witness")
        .arg("last")
        .arg("--count")
        .arg("1")
        .assert();
    let last_envelope = parse_stdout_json(&last_assert);
    common::assert_success_exit!(last_assert);
    assert_json_envelope_shape(&last_envelope);
    let last_records = last_envelope
        .get("result")
        .and_then(|r| r.get("records"))
        .and_then(|v| v.as_array())
        .expect("last records should be array");
    assert_eq!(last_records.len(), 1);
    assert_eq!(
        last_records[0].get("id").and_then(|v| v.as_str()),
        Some("blake3:second")
    );

    let query_assert = profile_cmd()
        .env("HOME", home)
        .arg("--json")
        .arg("witness")
        .arg("query")
        .arg("--limit")
        .arg("5")
        .assert();
    let query_envelope = parse_stdout_json(&query_assert);
    common::assert_success_exit!(query_assert);
    assert_json_envelope_shape(&query_envelope);
    let records = query_envelope
        .get("result")
        .and_then(|r| r.get("records"))
        .and_then(|v| v.as_array())
        .expect("query records should be array");
    assert_eq!(records.len(), 2);
    assert_eq!(
        records[0].get("id").and_then(|v| v.as_str()),
        Some("blake3:second")
    );
    assert_eq!(
        records[1].get("id").and_then(|v| v.as_str()),
        Some("blake3:first")
    );
}

#[test]
fn json_envelope_exposes_witness_id_only_when_append_succeeds() {
    let workspace = temp_workspace();
    let home = workspace.path();
    let dataset = workspace.path().join("dataset.csv");
    fs::write(&dataset, "loan_id,balance\nL1,100\nL2,200\n").expect("dataset should be written");

    let witnessed_assert = profile_cmd()
        .env("HOME", home)
        .arg("--json")
        .arg("stats")
        .arg(&dataset)
        .assert();
    let witnessed_envelope = parse_stdout_json(&witnessed_assert);
    common::assert_success_exit!(witnessed_assert);
    assert_json_envelope_shape(&witnessed_envelope);
    assert!(
        witnessed_envelope
            .get("witness_id")
            .and_then(|value| value.as_str())
            .is_some_and(|value| value.starts_with("blake3:"))
    );

    let no_witness_assert = profile_cmd()
        .env("HOME", home)
        .arg("--json")
        .arg("--no-witness")
        .arg("stats")
        .arg(&dataset)
        .assert();
    let no_witness_envelope = parse_stdout_json(&no_witness_assert);
    common::assert_success_exit!(no_witness_assert);
    assert_json_envelope_shape(&no_witness_envelope);
    assert!(
        no_witness_envelope
            .get("witness_id")
            .is_some_and(|value| value.is_null())
    );
}

fn ledger_line_count(home: &std::path::Path) -> usize {
    ledger_line_count_at(&home.join(".epistemic").join("witness.jsonl"))
}

fn ledger_line_count_at(path: &std::path::Path) -> usize {
    let content = fs::read_to_string(path).expect("witness ledger should exist");
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

fn read_ledger_record(path: &std::path::Path, index: usize) -> serde_json::Value {
    let content = fs::read_to_string(path).expect("witness ledger should exist");
    let line = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .nth(index)
        .expect("expected witness record");
    serde_json::from_str(line).expect("witness line should parse")
}
