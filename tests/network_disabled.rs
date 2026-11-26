//! bd-t1r: the default build has no network client; `push` and `pull` refuse E_NETWORK_DISABLED
//! (exit 2) before reading any file or touching the network. With the `network` feature the
//! real client exists and tests/push_pull.rs covers it.
#![cfg(not(feature = "network"))]

mod common;

use common::{fixture_path, parse_stdout_json, profile_cmd, temp_workspace};

#[test]
fn push_refuses_with_a_typed_code_before_reading_the_file() {
    // The file does not exist: a refusal about the missing file would mean push read it first.
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("push")
        .arg("/nonexistent/profile.yaml")
        .env("EPISTEMIC_FABRIC_URL", "http://127.0.0.1:9")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);
    assert_eq!(envelope["result"]["code"], "E_NETWORK_DISABLED");
    assert_eq!(envelope["result"]["detail"]["feature"], "network");
    assert!(
        envelope["result"]["next_command"]
            .as_str()
            .is_some_and(|next| next.contains("--features network"))
    );
}

#[test]
fn pull_refuses_with_a_typed_code_and_creates_nothing() {
    let workspace = temp_workspace();
    let out = workspace.path().join("pulled");
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("pull")
        .arg("csv.loan_tape.core.v0")
        .arg("--out")
        .arg(&out)
        .env("EPISTEMIC_FABRIC_URL", "http://127.0.0.1:9")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);
    assert_eq!(envelope["result"]["code"], "E_NETWORK_DISABLED");
    assert!(
        !out.exists(),
        "a refused pull must not create the output directory"
    );
}

#[test]
fn a_valid_frozen_profile_still_refuses_push_without_the_feature() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("push")
        .arg(fixture_path("profiles/valid/frozen_complete.yaml"))
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);
    assert_eq!(envelope["result"]["code"], "E_NETWORK_DISABLED");
}
