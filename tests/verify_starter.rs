//! bd-hoh: `profile emit-verify-starter` emits `verify`'s portable authoring format, the input of
//! `verify compile`. The format (verify 0.6.1, crates/verify-cli/src/compile/portable.rs) is a
//! `deny_unknown_fields` object: top level exactly `constraint_set_id`, `bindings` (a map of
//! `{key_fields?}`) and `rules` (each `id`, `severity`, `binding`, `op`, `columns`, ...).

mod common;

use std::fs;
use std::process::Command;

use common::{fixture_path, parse_stdout_json, profile_cmd, temp_workspace};
use serde_json::{Value, json};

const TOP_LEVEL: [&str; 3] = ["constraint_set_id", "bindings", "rules"];
/// verify's `PortableRuleAuthoring` fields (deny_unknown_fields).
const RULE_FIELDS: [&str; 12] = [
    "id",
    "severity",
    "portability",
    "binding",
    "op",
    "columns",
    "ref_binding",
    "ref_columns",
    "bindings",
    "query",
    "expr",
    "compare",
];
const PORTABLE_OPS: [&str; 6] = [
    "unique",
    "not_null",
    "predicate",
    "row_count",
    "aggregate_compare",
    "foreign_key",
];

/// Validate `doc` against the portable-authoring contract; every violation is returned.
fn authoring_violations(doc: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(object) = doc.as_object() else {
        return vec!["not an object".to_owned()];
    };
    for key in object.keys() {
        if !TOP_LEVEL.contains(&key.as_str()) {
            problems.push(format!("unknown top-level field '{key}'"));
        }
    }
    for key in TOP_LEVEL {
        if !object.contains_key(key) {
            problems.push(format!("missing top-level field '{key}'"));
        }
    }
    if !doc["constraint_set_id"]
        .as_str()
        .is_some_and(|s| !s.is_empty())
    {
        problems.push("constraint_set_id must be a non-empty string".to_owned());
    }
    let bindings = doc["bindings"].as_object();
    match bindings {
        Some(map) if !map.is_empty() => {
            for (name, binding) in map {
                let Some(binding) = binding.as_object() else {
                    problems.push(format!("binding '{name}' is not an object"));
                    continue;
                };
                for (field, value) in binding {
                    if field != "key_fields" {
                        problems.push(format!("binding '{name}': unknown field '{field}'"));
                    } else if !value
                        .as_array()
                        .is_some_and(|a| a.iter().all(Value::is_string))
                    {
                        problems.push(format!("binding '{name}': key_fields must be strings"));
                    }
                }
            }
        }
        _ => problems.push("bindings must be a non-empty object".to_owned()),
    }
    match doc["rules"].as_array() {
        Some(rules) if !rules.is_empty() => {
            for rule in rules {
                let Some(fields) = rule.as_object() else {
                    problems.push("rule is not an object".to_owned());
                    continue;
                };
                for field in fields.keys() {
                    if !RULE_FIELDS.contains(&field.as_str()) {
                        problems.push(format!("rule: unknown field '{field}'"));
                    }
                }
                for required in ["id", "severity", "op"] {
                    if !fields.contains_key(required) {
                        problems.push(format!("rule: missing '{required}'"));
                    }
                }
                if !rule["op"]
                    .as_str()
                    .is_some_and(|op| PORTABLE_OPS.contains(&op))
                {
                    problems.push(format!("rule: op {:?} is not a portable op", rule["op"]));
                }
                if !matches!(rule["severity"].as_str(), Some("error" | "warn")) {
                    problems.push(format!("rule: severity {:?}", rule["severity"]));
                }
                match (rule["binding"].as_str(), bindings) {
                    (Some(name), Some(map)) if map.contains_key(name) => {}
                    (name, _) => problems.push(format!("rule: binding {name:?} is not declared")),
                }
                if matches!(rule["op"].as_str(), Some("unique" | "not_null"))
                    && !rule["columns"].as_array().is_some_and(|c| !c.is_empty())
                {
                    problems.push("rule: columns must be non-empty".to_owned());
                }
            }
        }
        _ => problems.push("rules must be a non-empty array".to_owned()),
    }
    problems
}

/// Returns the exit code, the envelope, the `--out` file path and the workspace that owns it.
fn emit(args: &[&str]) -> (Option<i32>, Value, std::path::PathBuf, tempfile::TempDir) {
    let workspace = temp_workspace();
    let out = workspace.path().join("starter.json");
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("emit-verify-starter")
        .args(args)
        .arg("--out")
        .arg(&out)
        .assert();
    let code = assert.get_output().status.code();
    let envelope = parse_stdout_json(&assert);
    (code, envelope, out, workspace)
}

#[test]
fn the_validator_rejects_the_old_starter_shape() {
    // Planted negative: exactly what emit-verify-starter used to print (profile 0.11.0).
    let old = json!({
        "schema": "verify.rules.v1",
        "profile_id": "csv.loan_tape.core.v0",
        "profile_sha256": "sha256:0123",
        "key": ["loan_id"],
        "rules": [{"column": "loan_id", "type": "text", "nullable": false, "checks": ["not_null"]}]
    });
    let problems = authoring_violations(&old);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("unknown top-level field 'schema'")),
        "{problems:?}"
    );
    assert!(
        problems
            .iter()
            .any(|p| p.contains("unknown top-level field 'profile_id'"))
    );
    assert!(
        problems
            .iter()
            .any(|p| p.contains("missing top-level field 'constraint_set_id'"))
    );
    assert!(
        problems
            .iter()
            .any(|p| p.contains("unknown field 'column'"))
    );
}

#[test]
fn the_starter_is_valid_portable_authoring_with_no_unknown_fields() {
    let profile = fixture_path("profiles/valid/frozen_complete.yaml");
    let (code, envelope, file, _workspace) = emit(&[profile.to_str().unwrap()]);
    assert_eq!(code, Some(0), "{envelope}");
    let result = &envelope["result"];
    assert_eq!(
        authoring_violations(result),
        Vec::<String>::new(),
        "{result}"
    );
    // The file written by --out is the same document as the result.
    let written: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(&written, result);
    assert_eq!(result["constraint_set_id"], "csv.loan_tape.core.v0");
    assert_eq!(
        result["bindings"],
        json!({"input": {"key_fields": ["loan_id"]}})
    );
    // The profile's key is one unique rule over the key tuple.
    assert_eq!(
        result["rules"][0],
        json!({"id": "KEY_UNIQUE", "severity": "error", "binding": "input", "op": "unique", "columns": ["loan_id"]})
    );
    // Provenance moved to the envelope, not into the authoring document.
    assert_eq!(
        envelope["profile_ref"]["profile_id"],
        "csv.loan_tape.core.v0"
    );
}

#[test]
fn a_profile_with_nothing_to_derive_a_rule_from_is_refused_not_emitted_empty() {
    let workspace = temp_workspace();
    let keyless = workspace.path().join("keyless.yaml");
    let text = fs::read_to_string(fixture_path("profiles/valid/frozen_complete.yaml")).unwrap();
    // Remove the key (the profile hash is not verified by projections).
    let edited = text.replace("key:\n  - loan_id\n", "key: []\n");
    assert_ne!(edited, text);
    fs::write(&keyless, edited).unwrap();
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("emit-verify-starter")
        .arg(&keyless)
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);
    assert_eq!(envelope["result"]["code"], "E_INVALID_SCHEMA");
}

/// The shipped `verify` must accept the starter: `VERIFY_BIN=/path/to/verify cargo test` (verify
/// 0.6.1 or newer). Without the variable the structural test above is the check; the recorded
/// evidence for bd-hoh includes a run with the released 0.6.1 binary.
#[test]
fn the_released_verify_compiles_the_starter_when_verify_bin_is_set() {
    let Some(verify) = std::env::var_os("VERIFY_BIN") else {
        eprintln!("VERIFY_BIN not set: skipping the real `verify compile` run");
        return;
    };
    let profile = fixture_path("profiles/valid/frozen_complete.yaml");
    let (code, _, file, workspace) = emit(&[profile.to_str().unwrap()]);
    assert_eq!(code, Some(0));
    let output = Command::new(&verify)
        .arg("compile")
        .arg(&file)
        .output()
        .expect("run verify compile");
    assert_eq!(
        output.status.code(),
        Some(0),
        "verify compile refused the starter: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let compiled: Value = serde_json::from_slice(&output.stdout).expect("compiled json");
    assert_eq!(compiled["version"], "verify.constraint.v1");
    assert_eq!(compiled["constraint_set_id"], "csv.loan_tape.core.v0");

    // The real verify refuses the old shape, so the check above is not vacuous.
    let old = workspace.path().join("old.json");
    fs::write(
        &old,
        r#"{"schema":"verify.rules.v1","profile_id":"p","key":[],"rules":[{"column":"a","type":"text","nullable":false,"checks":[]}]}"#,
    )
    .unwrap();
    let refused = Command::new(&verify)
        .arg("compile")
        .arg(&old)
        .output()
        .unwrap();
    assert_eq!(refused.status.code(), Some(2));
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&refused.stdout),
        String::from_utf8_lossy(&refused.stderr)
    );
    assert!(
        said.contains("E_BAD_AUTHORING") && said.contains("unknown field"),
        "{said}"
    );
}
