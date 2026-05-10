mod common;

use common::{assert_json_envelope_shape, parse_stdout_json, profile_cmd, temp_workspace};

#[test]
fn doctor_health_json_reports_read_only_contract() {
    let assert = profile_cmd()
        .arg("doctor")
        .arg("health")
        .arg("--json")
        .assert();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert_eq!(stderr, "");
    assert_json_envelope_shape(&envelope);
    assert_eq!(
        envelope.get("subcommand").and_then(|entry| entry.as_str()),
        Some("doctor health")
    );
    assert_eq!(
        envelope.get("outcome").and_then(|entry| entry.as_str()),
        Some("SUCCESS")
    );

    let result = envelope.get("result").expect("result should be present");
    assert_eq!(
        result.get("contract").and_then(|entry| entry.as_str()),
        Some("cmdrvl.read_only_doctor.v1")
    );
    assert_eq!(
        result
            .get("tool")
            .and_then(|tool| tool.get("version"))
            .and_then(|entry| entry.as_str()),
        Some(env!("CARGO_PKG_VERSION"))
    );

    let side_effects = result
        .get("side_effects")
        .and_then(|entry| entry.as_object())
        .expect("side effects should be an object");
    for (key, value) in side_effects {
        assert_eq!(
            value.as_bool(),
            Some(false),
            "expected side effect {key} to be false"
        );
    }
}

#[test]
fn doctor_capabilities_json_lists_agent_surface() {
    let assert = profile_cmd()
        .arg("doctor")
        .arg("capabilities")
        .arg("--json")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);
    assert_json_envelope_shape(&envelope);

    assert_eq!(
        envelope.get("subcommand").and_then(|entry| entry.as_str()),
        Some("doctor capabilities")
    );

    let commands = envelope
        .get("result")
        .and_then(|result| result.get("commands"))
        .and_then(|entry| entry.as_array())
        .expect("capabilities should list commands");
    let names = commands
        .iter()
        .filter_map(|command| command.get("name").and_then(|entry| entry.as_str()))
        .collect::<Vec<_>>();

    assert!(names.contains(&"profile doctor health --json"));
    assert!(names.contains(&"profile doctor capabilities --json"));
    assert!(names.contains(&"profile doctor robot-docs"));
    assert!(names.contains(&"profile doctor --robot-triage"));

    assert_eq!(
        envelope
            .get("result")
            .and_then(|result| result.get("fix_mode"))
            .and_then(|fix_mode| fix_mode.get("available"))
            .and_then(|entry| entry.as_bool()),
        Some(false)
    );

    let detector_ids = envelope
        .get("result")
        .and_then(|result| result.get("detectors"))
        .and_then(|entry| entry.as_array())
        .expect("capabilities should list detector contracts")
        .iter()
        .filter_map(|detector| detector.get("id").and_then(|entry| entry.as_str()))
        .collect::<Vec<_>>();
    for expected in [
        "invalid_profile_schema",
        "dataset_column_mismatch",
        "already_frozen_profile",
        "witness_append_warning",
        "remote_push_transport_failure",
        "remote_pull_transport_failure",
    ] {
        assert!(detector_ids.contains(&expected));
    }
}

#[test]
fn doctor_robot_triage_is_machine_readable_without_global_json() {
    let assert = profile_cmd().arg("doctor").arg("--robot-triage").assert();
    let result = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert_eq!(
        result.get("schema").and_then(|entry| entry.as_str()),
        Some("profile.doctor.triage.v1")
    );
    assert_eq!(
        result.get("status").and_then(|entry| entry.as_str()),
        Some("healthy")
    );
    assert!(
        result
            .get("known_failure_modes")
            .and_then(|entry| entry.as_array())
            .is_some_and(|entries| entries.len() >= 3)
    );
}

#[test]
fn doctor_robot_docs_prints_plain_guidance() {
    let assert = profile_cmd().arg("doctor").arg("robot-docs").assert();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    common::assert_success_exit!(assert);

    assert_eq!(stderr, "");
    assert!(stdout.contains("profile doctor health --json"));
    assert!(stdout.contains("There is no doctor --fix mode."));
}

#[test]
fn doctor_fix_mode_is_not_available() {
    let assert = profile_cmd().arg("doctor").arg("--fix").assert();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    common::assert_refusal_exit!(assert);

    assert!(stderr.contains("unexpected argument '--fix'"));
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
