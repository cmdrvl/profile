mod common;

use std::fs;

use common::{
    assert_json_envelope_shape, copy_fixture, fixture_path, parse_stdout_json, profile_cmd,
    temp_workspace,
};

#[test]
fn show_json_resolves_existing_path_directly() {
    let workspace = temp_workspace();
    let profile_path = copy_fixture(
        "profiles/valid/frozen_complete.yaml",
        workspace.path().join("direct.yaml"),
    );
    let home_dir = workspace.path().join("home");
    fs::create_dir_all(&home_dir).expect("home directory should be created");

    let assert = profile_cmd()
        .env("HOME", &home_dir)
        .arg("--json")
        .arg("--no-witness")
        .arg("show")
        .arg(&profile_path)
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("path"))
            .and_then(|v| v.as_str()),
        Some(profile_path.to_string_lossy().as_ref())
    );
    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("profile"))
            .and_then(|p| p.get("profile_id"))
            .and_then(|v| v.as_str()),
        Some("csv.loan_tape.core.v0")
    );
}

#[test]
fn show_json_falls_back_to_home_profiles_by_profile_id() {
    let workspace = temp_workspace();
    let home_dir = workspace.path().join("home");
    let profiles_dir = home_dir.join(".epistemic").join("profiles");
    fs::create_dir_all(&profiles_dir).expect("profiles directory should be created");
    let profile_path = copy_fixture(
        "profiles/valid/frozen_complete.yaml",
        profiles_dir.join("from-home.yaml"),
    );

    let assert = profile_cmd()
        .env("HOME", &home_dir)
        .arg("--json")
        .arg("--no-witness")
        .arg("show")
        .arg("csv.loan_tape.core.v0")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert_eq!(
        envelope
            .get("result")
            .and_then(|r| r.get("path"))
            .and_then(|v| v.as_str()),
        Some(profile_path.to_string_lossy().as_ref())
    );
}

#[test]
fn show_json_refuses_unknown_profile_id_with_e_io() {
    let workspace = temp_workspace();
    let home_dir = workspace.path().join("home");
    fs::create_dir_all(&home_dir).expect("home directory should be created");

    let assert = profile_cmd()
        .env("HOME", &home_dir)
        .arg("--json")
        .arg("--no-witness")
        .arg("show")
        .arg("csv.unknown.profile.v9")
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

#[test]
fn show_human_output_contract_for_refusal() {
    let workspace = temp_workspace();
    let home_dir = workspace.path().join("home");
    fs::create_dir_all(&home_dir).expect("home directory should be created");

    profile_cmd()
        .env("HOME", &home_dir)
        .arg("show")
        .arg(fixture_path("profiles/invalid/malformed_yaml.yaml"))
        .assert()
        .code(common::EXIT_REFUSAL)
        .stderr(predicates::str::contains("Error:"))
        .stderr(predicates::str::contains("Profile fails schema validation"));
}
