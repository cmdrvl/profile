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
        "inputs": [],
        "params": { "subcommand": "lint" },
        "output_hash": "blake3:b",
        "outcome": "ISSUES_FOUND",
        "exit_code": 1,
        "prev": "blake3:first"
    });
    let mut contents = serde_json::to_string(&first).expect("first JSON");
    contents.push('\n');
    contents.push_str(&serde_json::to_string(&second).expect("second JSON"));
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
        Some(2)
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

fn ledger_line_count(home: &std::path::Path) -> usize {
    let path = home.join(".epistemic").join("witness.jsonl");
    let content = fs::read_to_string(path).expect("witness ledger should exist");
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}
