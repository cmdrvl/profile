mod common;

use common::{assert_json_envelope_shape, parse_stdout_json, profile_cmd};
use serde_json::Value;

#[test]
fn describe_human_emits_operator_manifest_without_subcommand() {
    let assert = profile_cmd().arg("--describe").assert();
    let manifest: Value = serde_json::from_slice(&assert.get_output().stdout)
        .expect("describe output should be JSON");
    common::assert_success_exit!(assert);

    assert_eq!(
        manifest.get("schema_version").and_then(|v| v.as_str()),
        Some("operator.v0")
    );
    assert_eq!(
        manifest.get("name").and_then(|v| v.as_str()),
        Some("profile")
    );
    assert_eq!(
        manifest
            .get("subcommands")
            .and_then(|v| v.as_array())
            .map(Vec::len),
        Some(13)
    );
}

#[test]
fn describe_json_wraps_operator_manifest_with_expected_fields() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--describe")
        .arg("--no-witness")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope.get("subcommand").and_then(|v| v.as_str()),
        Some("describe")
    );
    assert_eq!(
        envelope.get("outcome").and_then(|v| v.as_str()),
        Some("SUCCESS")
    );
    assert_eq!(envelope.get("exit_code").and_then(|v| v.as_i64()), Some(0));

    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("name"))
            .and_then(|v| v.as_str()),
        Some("profile")
    );
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("exit_codes"))
            .and_then(|codes| codes.get("1"))
            .and_then(|entry| entry.get("meaning"))
            .and_then(|v| v.as_str()),
        Some("ISSUES_FOUND")
    );
}

#[test]
fn describe_short_circuits_before_invalid_witness_args_are_parsed() {
    let assert = profile_cmd()
        .arg("--describe")
        .arg("witness")
        .arg("last")
        .arg("--count")
        .arg("nope")
        .assert();
    let manifest: Value = serde_json::from_slice(&assert.get_output().stdout)
        .expect("describe output should be JSON");
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    common::assert_success_exit!(assert);

    assert_eq!(
        manifest.get("schema_version").and_then(|v| v.as_str()),
        Some("operator.v0")
    );
    assert_eq!(stderr, "");
}

#[test]
fn schema_json_short_circuits_before_invalid_freeze_args_are_parsed() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--schema")
        .arg("freeze")
        .arg("missing.yaml")
        .arg("--version")
        .arg("nope")
        .arg("--family")
        .arg("csv.demo")
        .arg("--out")
        .arg("out.yaml")
        .assert();
    let envelope = parse_stdout_json(&assert);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    common::assert_success_exit!(assert);

    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope.get("subcommand").and_then(|v| v.as_str()),
        Some("schema")
    );
    assert_eq!(
        envelope
            .get("result")
            .and_then(|schema| schema.get("$id"))
            .and_then(|v| v.as_str()),
        Some("https://epistemic.so/schemas/profile.v1.json")
    );
    assert_eq!(stderr, "");
}
