mod common;

use std::fs;

use common::{assert_json_envelope_shape, parse_stdout_json, profile_cmd, temp_workspace};

#[test]
fn list_json_uses_home_profile_directory_and_sorts_family_then_version() {
    let workspace = temp_workspace();
    let home_dir = workspace.path().join("home");
    let profiles_dir = home_dir.join(".epistemic").join("profiles");
    fs::create_dir_all(&profiles_dir).expect("profiles directory should be created");

    write_frozen_profile(
        &profiles_dir.join("loan-v3.yaml"),
        "csv.loan_tape.core",
        3,
        "1111111111111111111111111111111111111111111111111111111111111111",
    );
    write_frozen_profile(
        &profiles_dir.join("alpha-v2.yaml"),
        "csv.alpha.core",
        2,
        "2222222222222222222222222222222222222222222222222222222222222222",
    );
    write_frozen_profile(
        &profiles_dir.join("loan-v1.yaml"),
        "csv.loan_tape.core",
        1,
        "3333333333333333333333333333333333333333333333333333333333333333",
    );
    fs::write(profiles_dir.join("notes.txt"), "ignore me\n").expect("extra file should be written");

    let assert = profile_cmd()
        .env("HOME", &home_dir)
        .arg("--json")
        .arg("--no-witness")
        .arg("list")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope.get("subcommand").and_then(|v| v.as_str()),
        Some("list")
    );

    let profiles = envelope
        .get("result")
        .and_then(|r| r.get("profiles"))
        .and_then(|v| v.as_array())
        .expect("result.profiles should be an array");
    let profile_ids = profiles
        .iter()
        .map(|profile| {
            profile
                .get("profile_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();

    assert_eq!(
        profile_ids,
        vec![
            "csv.alpha.core.v2",
            "csv.loan_tape.core.v1",
            "csv.loan_tape.core.v3"
        ]
    );
}

#[test]
fn list_json_returns_empty_when_home_profile_dir_is_missing() {
    let workspace = temp_workspace();
    let home_dir = workspace.path().join("empty-home");
    fs::create_dir_all(&home_dir).expect("home directory should be created");

    let assert = profile_cmd()
        .env("HOME", &home_dir)
        .arg("--json")
        .arg("--no-witness")
        .arg("list")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    let profiles = envelope
        .get("result")
        .and_then(|r| r.get("profiles"))
        .and_then(|v| v.as_array())
        .expect("result.profiles should be an array");
    assert!(profiles.is_empty());
}

fn write_frozen_profile(path: &std::path::Path, family: &str, version: u64, sha_hex: &str) {
    let profile_id = format!("{family}.v{version}");
    let content = format!(
        "\
schema_version: 1
profile_id: {profile_id}
profile_version: {version}
profile_family: {family}
profile_sha256: sha256:{sha_hex}
status: frozen
format: csv
include_columns:
  - loan_id
",
    );
    fs::write(path, content).expect("profile fixture should be written");
}
