mod common;

use common::{assert_json_envelope_shape, fixture_path, parse_stdout_json, profile_cmd};

#[test]
fn suggest_key_json_is_deterministic_and_applies_position_tiebreak() {
    let assert_one = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("suggest-key")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .assert();
    let envelope_one = parse_stdout_json(&assert_one);
    common::assert_success_exit!(assert_one);

    let assert_two = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("suggest-key")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .assert();
    let envelope_two = parse_stdout_json(&assert_two);
    common::assert_success_exit!(assert_two);

    assert_eq!(envelope_one.get("result"), envelope_two.get("result"));
    assert_json_envelope_shape(&envelope_one);
    assert_eq!(
        envelope_one.get("subcommand").and_then(|v| v.as_str()),
        Some("suggest-key")
    );

    let candidates = envelope_one
        .get("result")
        .and_then(|r| r.get("candidates"))
        .and_then(|v| v.as_array())
        .expect("suggest-key result should contain candidates array");
    let names = candidates
        .iter()
        .map(|candidate| {
            candidate
                .get("column")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["loan_id", "balance", "rate", "property_type"]);

    assert_eq!(
        candidates[1].get("rank").and_then(|v| v.as_i64()),
        Some(2),
        "balance should outrank rate by position tie-break",
    );
    assert_eq!(candidates[2].get("rank").and_then(|v| v.as_i64()), Some(3));
}

#[test]
fn suggest_key_json_respects_top_limit() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("suggest-key")
        .arg(fixture_path("datasets/valid/loan_tape_basic.csv"))
        .arg("--top")
        .arg("2")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    let candidates = envelope
        .get("result")
        .and_then(|r| r.get("candidates"))
        .and_then(|v| v.as_array())
        .expect("suggest-key result should contain candidates array");
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].get("rank").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(candidates[1].get("rank").and_then(|v| v.as_i64()), Some(2));
}

#[test]
fn suggest_key_json_refuses_malformed_csv_with_e_csv_parse() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("suggest-key")
        .arg(fixture_path("datasets/invalid/malformed_quotes.csv"))
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("code"))
            .and_then(|v| v.as_str()),
        Some("E_CSV_PARSE")
    );
}

#[test]
fn suggest_key_json_refuses_header_only_dataset_with_e_empty() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("suggest-key")
        .arg(fixture_path("datasets/invalid/header_only.csv"))
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("code"))
            .and_then(|v| v.as_str()),
        Some("E_EMPTY")
    );
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("detail"))
            .and_then(|d| d.get("reason"))
            .and_then(|v| v.as_str()),
        Some("no data rows")
    );
}
