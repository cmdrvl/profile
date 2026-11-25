mod common;

use common::{assert_json_envelope_shape, parse_stdout_json, profile_cmd};
use serde_json::Value;

#[test]
fn describe_human_emits_operator_manifest_without_subcommand() {
    let assert = profile_cmd().arg("--describe").assert();
    let manifest: Value = serde_json::from_slice(&assert.get_output().stdout)
        .expect("describe output should be JSON");
    common::assert_success_exit!(assert);

    assert_v1_manifest(&manifest);
    // The exact command inventory, in order (stronger than the old bare count of 18; `evolve`
    // was added by profile evolve, 624280c, making 19).
    let names: Vec<&str> = manifest
        .get("commands")
        .and_then(|v| v.as_array())
        .expect("commands array")
        .iter()
        .filter_map(|command| command.get("name").and_then(|v| v.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            "doctor",
            "capabilities",
            "robot-docs",
            "draft new",
            "draft init",
            "validate",
            "lint",
            "evolve",
            "slice",
            "emit-discovery",
            "emit-verify-starter",
            "emit-ddl",
            "stats",
            "suggest-key",
            "freeze",
            "list",
            "show",
            "diff",
            "push",
            "pull",
            "witness"
        ]
    );
    for expected in ["slice", "emit-discovery", "capabilities", "robot-docs"] {
        assert!(
            manifest
                .get("commands")
                .and_then(|v| v.as_array())
                .is_some_and(|commands| {
                    commands.iter().any(|command| {
                        command.get("name").and_then(|v| v.as_str()) == Some(expected)
                    })
                }),
            "operator manifest should include top-level {expected}"
        );
    }
}

/// operator.v1 identity: schema_version operator.v1, `tool_version` (v1's rename of the v0
/// `version`) equal to the crate version, and none of the retired v0 spellings.
fn assert_v1_manifest(manifest: &Value) {
    assert_eq!(
        manifest.get("schema_version").and_then(|v| v.as_str()),
        Some("operator.v1")
    );
    assert_eq!(
        manifest.get("name").and_then(|v| v.as_str()),
        Some("profile")
    );
    assert_eq!(
        manifest.get("tool_version").and_then(|v| v.as_str()),
        Some(env!("CARGO_PKG_VERSION")),
        "--describe tool_version must stay aligned with Cargo.toml"
    );
    for retired in ["version", "subcommands"] {
        assert!(
            manifest.get(retired).is_none(),
            "operator.v1 must not carry the v0 field `{retired}`"
        );
    }
}

#[test]
fn describe_json_wraps_operator_manifest_with_expected_fields() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("--describe")
        .arg("--no-witness")
        .assert();
    let manifest = parse_stdout_json(&assert);
    let json_stdout = assert.get_output().stdout.clone();
    common::assert_success_exit!(assert);

    // operator.v1: `--describe` prints the manifest itself, with or without --json. The old
    // profile.v0 wrapper (version/outcome/exit_code/subcommand/result) is gone, and its
    // absence is pinned so a wrapper cannot silently come back.
    for wrapper_key in ["outcome", "exit_code", "subcommand", "result", "witness_id"] {
        assert!(
            manifest.get(wrapper_key).is_none(),
            "--describe --json must not wrap the manifest (found `{wrapper_key}`)"
        );
    }
    assert_v1_manifest(&manifest);
    assert_eq!(
        manifest
            .get("exit_codes")
            .and_then(|codes| codes.get("1"))
            .and_then(|entry| entry.get("meaning"))
            .and_then(|v| v.as_str()),
        Some("ISSUES_FOUND")
    );

    // Same bytes as plain --describe.
    let plain = profile_cmd().arg("--describe").assert();
    assert_eq!(json_stdout, plain.get_output().stdout);
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

    assert_v1_manifest(&manifest);
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
    assert_eq!(envelope["result"]["properties"]["key"]["uniqueItems"], true);
    assert_eq!(
        envelope["result"]["properties"]["key"]["items"]["minLength"],
        1
    );
    assert_eq!(
        envelope["result"]["properties"]["column_registry_hash"]["pattern"],
        "^blake3:[a-f0-9]{64}$"
    );
    assert_eq!(
        envelope["result"]["properties"]["format"]["enum"],
        serde_json::json!(["csv", "tsv", "parquet", "jsonl", "xlsx"])
    );
    assert!(envelope["result"]["properties"]["columns"].is_object());
    assert_eq!(stderr, "");
}

#[test]
fn schema_json_supports_discovery_schema_short_circuit() {
    let assert = profile_cmd()
        .arg("--json")
        .arg("emit-discovery")
        .arg("--schema")
        .arg("missing-sliced.csv")
        .arg("--source-file")
        .arg("missing-source.csv")
        .arg("--skip-rows")
        .arg("4")
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
        Some("https://epistemic.so/schemas/profile.discovery.v0.json")
    );
    assert_eq!(stderr, "");
}
