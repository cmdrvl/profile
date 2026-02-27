mod common;

use std::fs;

use common::{
    assert_json_envelope_shape, fixture_path, parse_stdout_json, profile_cmd, temp_workspace,
};
use serde_yaml::Value as YamlValue;

#[test]
fn freeze_writes_deterministic_golden_profile_and_hash() {
    let workspace = temp_workspace();
    let out_one = workspace.path().join("frozen-one.yaml");
    let out_two = workspace.path().join("frozen-two.yaml");

    let assert_one = profile_cmd()
        .arg("--no-witness")
        .arg("freeze")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .arg("--family")
        .arg("csv.loan_tape.core")
        .arg("--version")
        .arg("0")
        .arg("--out")
        .arg(&out_one)
        .assert();
    common::assert_success_exit!(assert_one);

    let frozen_one = fs::read_to_string(&out_one).expect("frozen profile should be readable");
    let expected = "\
schema_version: 1
profile_id: csv.loan_tape.core.v0
profile_version: 0
profile_family: csv.loan_tape.core
profile_sha256: sha256:79dfeeb23cda6d894d756c84e7aca1b244dd7a8ab4ed24aed44908589635e5bf
status: frozen
format: csv
hashing:
  algorithm: sha256
equivalence:
  order: order-invariant
  float_decimals: 6
  trim_strings: true
key:
- loan_id
include_columns:
- loan_id
- balance
- rate
";
    assert_eq!(frozen_one, expected);

    let assert_two = profile_cmd()
        .arg("--no-witness")
        .arg("freeze")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .arg("--family")
        .arg("csv.loan_tape.core")
        .arg("--version")
        .arg("0")
        .arg("--out")
        .arg(&out_two)
        .assert();
    common::assert_success_exit!(assert_two);

    let frozen_two =
        fs::read_to_string(&out_two).expect("second frozen profile should be readable");
    assert_eq!(frozen_one, frozen_two);
}

#[test]
fn freeze_json_success_uses_unified_output_envelope() {
    let workspace = temp_workspace();
    let out_path = workspace.path().join("freeze.json.yaml");

    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("freeze")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .arg("--family")
        .arg("csv.loan_tape.core")
        .arg("--version")
        .arg("5")
        .arg("--out")
        .arg(&out_path)
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope.get("subcommand").and_then(|v| v.as_str()),
        Some("freeze")
    );
    assert_eq!(
        envelope.get("outcome").and_then(|v| v.as_str()),
        Some("SUCCESS")
    );
    assert_eq!(envelope.get("exit_code").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("path"))
            .and_then(|v| v.as_str()),
        Some(out_path.to_string_lossy().as_ref())
    );
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("profile_id"))
            .and_then(|v| v.as_str()),
        Some("csv.loan_tape.core.v5")
    );

    let sha = envelope
        .get("result")
        .and_then(|r| r.get("profile_sha256"))
        .and_then(|v| v.as_str())
        .expect("profile_sha256 should be present");
    assert!(sha.starts_with("sha256:"));
    assert_eq!(sha.len(), "sha256:".len() + 64);
}

#[test]
fn freeze_applies_defaults_for_minimal_draft() {
    let workspace = temp_workspace();
    let out_path = workspace.path().join("frozen-minimal.yaml");

    let assert = profile_cmd()
        .arg("--no-witness")
        .arg("freeze")
        .arg(fixture_path("profiles/valid/draft_minimal.yaml"))
        .arg("--family")
        .arg("csv.loan_tape.min")
        .arg("--version")
        .arg("3")
        .arg("--out")
        .arg(&out_path)
        .assert();
    common::assert_success_exit!(assert);

    let content = fs::read_to_string(&out_path).expect("frozen profile should be readable");
    let yaml: YamlValue = serde_yaml::from_str(&content).expect("frozen profile YAML should parse");

    assert_eq!(yaml["hashing"]["algorithm"].as_str(), Some("sha256"));
    assert_eq!(
        yaml["equivalence"]["order"].as_str(),
        Some("order-invariant")
    );
    assert_eq!(yaml["profile_id"].as_str(), Some("csv.loan_tape.min.v3"));
}

#[test]
fn freeze_json_refuses_bad_family_with_e_bad_version() {
    let workspace = temp_workspace();
    let out_path = workspace.path().join("bad-family.yaml");

    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("freeze")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .arg("--family")
        .arg("Csv.bad")
        .arg("--version")
        .arg("0")
        .arg("--out")
        .arg(&out_path)
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("code"))
            .and_then(|v| v.as_str()),
        Some("E_BAD_VERSION")
    );
}

#[test]
fn freeze_json_refuses_already_frozen_input_with_e_already_frozen() {
    let workspace = temp_workspace();
    let out_path = workspace.path().join("already-frozen.yaml");

    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("freeze")
        .arg(fixture_path("profiles/valid/frozen_complete.yaml"))
        .arg("--family")
        .arg("csv.loan_tape.core")
        .arg("--version")
        .arg("0")
        .arg("--out")
        .arg(&out_path)
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("code"))
            .and_then(|v| v.as_str()),
        Some("E_ALREADY_FROZEN")
    );
}

#[test]
fn freeze_json_refuses_existing_output_path_with_e_io() {
    let workspace = temp_workspace();
    let out_path = workspace.path().join("exists.yaml");
    fs::write(&out_path, "placeholder\n").expect("placeholder file should be written");

    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("freeze")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .arg("--family")
        .arg("csv.loan_tape.core")
        .arg("--version")
        .arg("4")
        .arg("--out")
        .arg(&out_path)
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);

    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("code"))
            .and_then(|v| v.as_str()),
        Some("E_IO")
    );
}
