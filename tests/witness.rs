mod common;

use std::fs;

use common::{assert_json_envelope_shape, parse_stdout_json, profile_cmd, temp_workspace};
use serde_json::{Value, json};

#[test]
fn source_date_epoch_makes_witness_timestamp_reproducible() {
    let workspace = temp_workspace();
    let home = workspace.path().join("home");
    let ledger = workspace.path().join("state").join("witness.jsonl");
    let dataset = workspace.path().join("dataset.csv");
    fs::write(&dataset, "loan_id,balance\nL1,100\n").expect("dataset should be written");

    let assert = profile_cmd()
        .env("HOME", &home)
        .env("EPISTEMIC_WITNESS", &ledger)
        .env("SOURCE_DATE_EPOCH", "1700000000")
        .arg("stats")
        .arg(&dataset)
        .assert();
    common::assert_success_exit!(assert);

    let record = read_ledger_record(&ledger, 0);
    assert_eq!(
        record.get("ts").and_then(|value| value.as_str()),
        Some("2023-11-14T22:13:20Z")
    );
}

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
    assert!(!canonical_witness_path(home).exists());
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
    let count_result = parse_stdout_json(&count_assert);
    common::assert_success_exit!(count_assert);
    assert_eq!(count_result.get("count").and_then(|v| v.as_u64()), Some(1));
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
    let count_result = parse_stdout_json(&count_assert);
    common::assert_success_exit!(count_assert);
    assert!(canonical_witness_path(home).exists());
    let migration = fs::read_to_string(home.join(".cmdrvl/migrations/applied.jsonl")).unwrap();
    assert!(migration.contains("\"path_class\":\"witness_ledger\""));
    assert_eq!(
        count_result.get("count").and_then(|v| v.as_u64()),
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
    let last_record = parse_stdout_json(&last_assert);
    common::assert_success_exit!(last_assert);
    assert_eq!(last_record, second);
    assert_eq!(
        last_record.get("id").and_then(|v| v.as_str()),
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
    let records = parse_stdout_json(&query_assert);
    common::assert_success_exit!(query_assert);
    let records = records.as_array().expect("query records should be array");
    assert_eq!(records, &[second, first]);
    assert_eq!(records.len(), 2);
    assert_eq!(
        records[0].get("id").and_then(|v| v.as_str()),
        Some("blake3:second")
    );
    assert_eq!(
        records[1].get("id").and_then(|v| v.as_str()),
        Some("blake3:first")
    );

    let limited_assert = profile_cmd()
        .env("HOME", home)
        .arg("--json")
        .arg("witness")
        .arg("query")
        .arg("--limit")
        .arg("1")
        .assert();
    let limited_records = parse_stdout_json(&limited_assert);
    common::assert_success_exit!(limited_assert);
    assert_eq!(
        limited_records,
        json!([{
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
        }])
    );
}

#[test]
fn witness_json_reads_use_bare_family_shapes_for_absent_empty_and_populated_ledgers() {
    let workspace = temp_workspace();
    let home = workspace.path().join("home");
    let ledger_path = workspace.path().join("witness.jsonl");

    for state in ["absent", "empty", "populated"] {
        if state == "empty" {
            fs::write(&ledger_path, "").expect("empty ledger should be written");
        } else if state == "populated" {
            let first = json!({
                "id": "blake3:first",
                "tool": "profile",
                "version": "0.0.0-test",
                "params": { "subcommand": "stats" },
            });
            let second = json!({
                "id": "blake3:second",
                "tool": "profile",
                "version": "0.0.0-test",
                "params": { "subcommand": "lint" },
            });
            fs::write(
                &ledger_path,
                format!(
                    "{}\n{}\n",
                    serde_json::to_string(&first).unwrap(),
                    serde_json::to_string(&second).unwrap()
                ),
            )
            .expect("populated ledger should be written");
        }

        let query = run_json_witness(&home, &ledger_path, &["witness", "query"]);
        let count = run_json_witness(&home, &ledger_path, &["witness", "count"]);
        let last = run_json_witness(&home, &ledger_path, &["witness", "last"]);
        let last_one = run_json_witness(&home, &ledger_path, &["witness", "last", "--count", "1"]);
        let last_two = run_json_witness(&home, &ledger_path, &["witness", "last", "--count", "2"]);
        let last_zero = run_json_witness(&home, &ledger_path, &["witness", "last", "--count", "0"]);

        match state {
            "populated" => {
                assert_eq!(
                    query,
                    json!([{"id": "blake3:second", "tool": "profile", "version": "0.0.0-test", "params": {"subcommand": "lint"}}, {"id": "blake3:first", "tool": "profile", "version": "0.0.0-test", "params": {"subcommand": "stats"}}])
                );
                assert_eq!(count, json!({"count": 2}));
                assert_eq!(
                    last,
                    json!({"id": "blake3:second", "tool": "profile", "version": "0.0.0-test", "params": {"subcommand": "lint"}})
                );
                assert_eq!(last_one, last);
                assert_eq!(
                    last_two,
                    json!([{"id": "blake3:second", "tool": "profile", "version": "0.0.0-test", "params": {"subcommand": "lint"}}, {"id": "blake3:first", "tool": "profile", "version": "0.0.0-test", "params": {"subcommand": "stats"}}])
                );
            }
            "absent" | "empty" => {
                assert_eq!(query, json!([]));
                assert_eq!(count, json!({"count": 0}));
                assert_eq!(last, Value::Null);
                assert_eq!(last_one, Value::Null);
                assert_eq!(last_two, json!([]));
            }
            _ => unreachable!("test state is fixed above"),
        }
        assert_eq!(last_zero, json!([]));

        if state == "absent" {
            assert!(
                !ledger_path.exists(),
                "absent state must not create a ledger"
            );
        }
        if state == "populated" {
            fs::remove_file(&ledger_path).expect("reset ledger between states");
        }
    }
}

fn run_json_witness(
    home: &std::path::Path,
    ledger_path: &std::path::Path,
    args: &[&str],
) -> serde_json::Value {
    let output = profile_cmd()
        .env("HOME", home)
        .env("EPISTEMIC_WITNESS", ledger_path)
        .arg("--json")
        .args(args)
        .output()
        .expect("witness command should start");
    assert!(
        output.status.success(),
        "witness {args:?} should exit 0: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("witness stdout should be JSON")
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

#[test]
fn witness_json_refusals_keep_the_profile_envelope() {
    let workspace = temp_workspace();
    let home = workspace.path().join("home");
    let ledger_path = workspace.path().join("witness.jsonl");
    fs::write(&ledger_path, "not-json\n").expect("invalid ledger should be written");

    let assert = profile_cmd()
        .env("HOME", &home)
        .env("EPISTEMIC_WITNESS", &ledger_path)
        .arg("--json")
        .arg("witness")
        .arg("query")
        .assert();
    let refusal = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);
    assert_eq!(
        refusal.get("version").and_then(Value::as_str),
        Some("profile.v0")
    );
    assert_eq!(
        refusal.get("outcome").and_then(Value::as_str),
        Some("REFUSAL")
    );
    assert_eq!(
        refusal.get("exit_code").and_then(|value| value.as_u64()),
        Some(2)
    );
    assert_eq!(
        refusal
            .get("result")
            .and_then(|result| result.get("code"))
            .and_then(Value::as_str),
        Some("E_INVALID_SCHEMA")
    );
}

fn ledger_line_count(home: &std::path::Path) -> usize {
    ledger_line_count_at(&canonical_witness_path(home))
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

fn canonical_witness_path(home: &std::path::Path) -> std::path::PathBuf {
    home.join(".cmdrvl")
        .join("state")
        .join("witness")
        .join("witness.jsonl")
}
