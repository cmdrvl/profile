mod common;

use common::{parse_stdout_json, profile_cmd, temp_workspace};

#[test]
fn top_level_robot_triage_is_machine_readable_without_global_json() {
    let assert = profile_cmd().arg("--robot-triage").assert();
    let result = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert_eq!(
        result
            .get("schema_version")
            .and_then(|entry| entry.as_str()),
        Some("cmdrvl.doctor.v1")
    );
    assert_eq!(
        result.get("mode").and_then(|entry| entry.as_str()),
        Some("triage")
    );
    assert_eq!(
        result.get("healthy").and_then(|entry| entry.as_bool()),
        Some(true)
    );
    assert!(
        result
            .get("next_actions")
            .and_then(|entry| entry.as_array())
            .is_some()
    );
}

#[test]
fn top_level_capabilities_json_lists_agent_surfaces() {
    let assert = profile_cmd().arg("capabilities").arg("--json").assert();
    let capabilities = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);
    assert_eq!(
        capabilities
            .get("schema_version")
            .and_then(|entry| entry.as_str()),
        Some("operator.v1")
    );
    assert_eq!(
        capabilities.get("name").and_then(|entry| entry.as_str()),
        Some("profile")
    );
    assert!(
        capabilities
            .get("commands")
            .and_then(|entry| entry.as_array())
            .is_some()
    );

    let described = profile_cmd().arg("--describe").assert();
    let described = parse_stdout_json(&described);
    assert_eq!(capabilities, described);
}

#[test]
fn top_level_robot_docs_guide_prints_plain_guidance() {
    let assert = profile_cmd().arg("robot-docs").arg("guide").assert();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    common::assert_success_exit!(assert);

    assert_eq!(stderr, "");
    assert!(stdout.contains("profile --robot-triage"));
    assert!(stdout.contains("profile capabilities --json"));
    assert!(stdout.contains("profile robot-docs guide"));
    assert!(stdout.contains("Composition:"));
    assert!(stdout.contains("profile draft init tape.csv --from-peek tape.peek.json"));
    assert!(stdout.contains("shape old.csv new.csv --profile profiles/csv.loan_tape.core.v0.yaml"));
    assert!(stdout.contains("profile doctor --fix is unavailable"));
}

#[test]
fn help_is_ansi_free_with_no_color_and_non_tty_environment() {
    let assert = profile_cmd()
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .arg("--help")
        .assert();
    let stdout_ansi_free = !assert.get_output().stdout.contains(&0x1b);
    let stderr_ansi_free = !assert.get_output().stderr.contains(&0x1b);
    common::assert_success_exit!(assert);
    assert!(stdout_ansi_free);
    assert!(stderr_ansi_free);
}

#[test]
fn output_alias_is_accepted_for_profile_artifacts() {
    let workspace = temp_workspace();
    let output = workspace.path().join("draft.yaml");
    let assert = profile_cmd()
        .arg("draft")
        .arg("new")
        .arg("--output")
        .arg(&output)
        .assert();
    common::assert_success_exit!(assert);
    assert!(output.exists());
}

#[test]
fn doctor_health_json_reports_read_only_contract() {
    let assert = profile_cmd()
        .arg("doctor")
        .arg("health")
        .arg("--json")
        .assert();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    let report = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert_eq!(stderr, "");
    assert_eq!(
        report
            .get("schema_version")
            .and_then(|entry| entry.as_str()),
        Some("cmdrvl.doctor.v1")
    );
    assert_eq!(
        report.get("mode").and_then(|entry| entry.as_str()),
        Some("health")
    );
    assert_eq!(
        report.get("tool").and_then(|entry| entry.as_str()),
        Some("profile")
    );
    assert_eq!(
        report.get("tool_version").and_then(|entry| entry.as_str()),
        Some(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(
        report.get("healthy").and_then(|entry| entry.as_bool()),
        Some(true)
    );
    assert!(
        report
            .get("findings")
            .and_then(|entry| entry.as_array())
            .is_some()
    );
}

#[test]
fn doctor_capabilities_json_lists_agent_surface() {
    let assert = profile_cmd()
        .arg("doctor")
        .arg("capabilities")
        .arg("--json")
        .assert();
    let report = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);
    assert_eq!(
        report
            .get("schema_version")
            .and_then(|entry| entry.as_str()),
        Some("cmdrvl.doctor.v1")
    );
    assert_eq!(
        report.get("mode").and_then(|entry| entry.as_str()),
        Some("capabilities")
    );
    let contract = report
        .get("contract")
        .expect("capabilities report should embed contract");
    assert_eq!(
        contract
            .get("schema_version")
            .and_then(|entry| entry.as_str()),
        Some("operator.v1")
    );
    assert_eq!(
        contract.get("name").and_then(|entry| entry.as_str()),
        Some("profile")
    );
}

#[test]
fn doctor_robot_triage_is_machine_readable_without_global_json() {
    let assert = profile_cmd().arg("doctor").arg("--robot-triage").assert();
    let result = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert_eq!(
        result
            .get("schema_version")
            .and_then(|entry| entry.as_str()),
        Some("cmdrvl.doctor.v1")
    );
    assert_eq!(
        result.get("mode").and_then(|entry| entry.as_str()),
        Some("triage")
    );
    assert!(
        result
            .get("next_actions")
            .and_then(|entry| entry.as_array())
            .is_some()
    );
}

#[test]
fn doctor_robot_docs_prints_plain_guidance() {
    let assert = profile_cmd().arg("doctor").arg("robot-docs").assert();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    common::assert_success_exit!(assert);

    assert_eq!(stderr, "");
    assert!(stdout.contains("profile --robot-triage"));
    assert!(stdout.contains("profile capabilities --json"));
    assert!(stdout.contains("profile robot-docs guide"));
    assert!(stdout.contains("profile doctor health --json"));
    assert!(stdout.contains("profile doctor --fix is unavailable"));
}

#[test]
fn doctor_fix_mode_is_not_available() {
    let assert = profile_cmd().arg("doctor").arg("--fix").assert();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    common::assert_refusal_exit!(assert);

    assert_eq!(stdout, "");
    assert!(stderr.contains("profile doctor --fix is unavailable"));
    assert!(stderr.contains("profile --robot-triage"));
    assert!(stderr.contains("profile capabilities --json"));
    assert!(stderr.contains("profile robot-docs guide"));
}

#[test]
fn doctor_fix_subcommand_is_not_available() {
    let assert = profile_cmd().arg("doctor").arg("fix").assert();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    common::assert_refusal_exit!(assert);

    assert_eq!(stdout, "");
    assert!(stderr.contains("profile doctor --fix is unavailable"));
    assert!(stderr.contains("profile --robot-triage"));
}

#[test]
fn doctor_does_not_write_local_artifacts() {
    let workspace = temp_workspace();

    let assert = profile_cmd()
        .current_dir(workspace.path())
        .arg("doctor")
        .arg("health")
        .arg("--json")
        .assert();
    common::assert_success_exit!(assert);

    assert!(!workspace.path().join(".doctor").exists());
}
