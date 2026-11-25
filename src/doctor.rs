use serde_json::{Value, json};

use crate::cli::args::{DoctorArgs, DoctorCommand};
use crate::cli::exit::EXIT_REFUSAL;
use crate::refusal::RefusalPayload;

const CONTRACT: &str = "cmdrvl.read_only_doctor.v1";
const TOOL_ROLE: &str = "configuration authoring tool for deterministic column-scoping profiles";

pub fn run(args: &DoctorArgs) -> Result<Value, RefusalPayload> {
    if args.fix {
        return Err(fix_unavailable_refusal());
    }

    if args.robot_triage {
        return Ok(triage_report());
    }

    match args.command.as_ref().unwrap_or(&DoctorCommand::Health) {
        DoctorCommand::Health => Ok(health_report()),
        DoctorCommand::Capabilities => Ok(capabilities_report()),
        DoctorCommand::Triage(_) => Ok(triage_report()),
        DoctorCommand::RobotDocs => Ok(robot_docs()),
        DoctorCommand::Fix => Err(fix_unavailable_refusal()),
    }
}

pub fn emit_fix_unavailable() -> u8 {
    eprintln!("profile doctor --fix is unavailable: diagnostics are read-only in this release.");
    eprintln!("Try --robot-triage: profile --robot-triage");
    eprintln!("Try capabilities --json: profile capabilities --json");
    eprintln!("Try robot-docs guide: profile robot-docs guide");
    EXIT_REFUSAL
}

fn fix_unavailable_refusal() -> RefusalPayload {
    RefusalPayload::simple(
        "E_INVALID_SCHEMA",
        "profile doctor --fix is unavailable: diagnostics are read-only in this release.",
    )
    .with_next_command("profile --robot-triage")
}

fn tool_metadata() -> Value {
    json!({
        "name": "profile",
        "version": env!("CARGO_PKG_VERSION"),
        "role": TOOL_ROLE,
        "primary_inputs": [
            "profile YAML",
            "CSV dataset headers",
            "optional column registry"
        ],
        "primary_outputs": [
            "draft profile YAML",
            "frozen profile YAML",
            "lint and statistics reports",
            "witness records for deterministic profile operations"
        ],
        "downstream_consumers": [
            "shape",
            "compare",
            "rvl"
        ]
    })
}

fn side_effects() -> Value {
    json!({
        "reads_stdin": false,
        "reads_profile_files": false,
        "reads_dataset_files": false,
        "reads_column_registries": false,
        "reads_witness_ledger": false,
        "resolves_profile_ids": false,
        "validates_profile_schema": false,
        "lints_dataset_columns": false,
        "computes_profile_hash": false,
        "writes_profile_files": false,
        "writes_witness_ledger": false,
        "writes_doctor_artifacts": false,
        "uses_network": false,
        "changes_cwd": false
    })
}

fn domain_boundaries() -> Value {
    json!({
        "owns": [
            "draft profile YAML authoring",
            "profile schema validation",
            "profile-to-dataset column linting",
            "deterministic profile freezing and SHA256 identity",
            "profile resolution, listing, and structural diffing",
            "ambient witness records for supported deterministic operations"
        ],
        "does_not_own": [
            "downstream report comparison semantics",
            "rvl row-level reconciliation",
            "shape structural dataset diffing",
            "automatic profile repair",
            "profile content migration across versions",
            "remote data-fabric availability"
        ]
    })
}

fn health_report() -> Value {
    json!({
        "schema": "profile.doctor.health.v1",
        "contract": CONTRACT,
        "status": "healthy",
        "healthy": true,
        "tool": tool_metadata(),
        "checks": [
            {
                "id": "cli_loaded",
                "status": "pass",
                "detail": "profile CLI metadata is available"
            },
            {
                "id": "doctor_read_only",
                "status": "pass",
                "detail": "doctor dispatch returns before profile, dataset, registry, witness, or network handlers"
            },
            {
                "id": "fix_mode_disabled",
                "status": "pass",
                "detail": "profile doctor --fix refuses safely and names read-only alternatives"
            },
            {
                "id": "output_contract_preserved",
                "status": "pass",
                "detail": "doctor --json uses the existing profile.v0 output envelope"
            },
            {
                "id": "domain_boundary_preserved",
                "status": "pass",
                "detail": "doctor does not validate, lint, freeze, resolve, diff, push, pull, or append witness records"
            },
            {
                "id": "fixture_backed_detectors_declared",
                "status": "pass",
                "detail": "known profile failure modes are declared as detector-only coverage before any fix surface exists"
            }
        ],
        "detectors": detector_contracts(),
        "observed_inputs": {
            "profiles": [],
            "datasets": [],
            "column_registries": [],
            "witness_ledger": null,
            "network_endpoint": null
        },
        "side_effects": side_effects(),
        "domain_boundaries": domain_boundaries()
    })
}

pub fn capabilities_report() -> Value {
    json!({
        "schema": "profile.doctor.capabilities.v1",
        "contract": CONTRACT,
        "status": "available",
        "tool": tool_metadata(),
        "agent_surfaces": {
            "robot_triage": {
                "command": "profile --robot-triage",
                "output": "profile.doctor.triage.v1 JSON diagnostic report",
                "mutates": false
            },
            "capabilities": {
                "command": "profile capabilities --json",
                "output": "profile.doctor.capabilities.v1 inside the profile.v0 envelope",
                "mutates": false
            },
            "robot_docs": {
                "command": "profile robot-docs guide",
                "output": "agent-oriented operating guide",
                "mutates": false
            },
            "doctor_namespace": {
                "commands": [
                    "profile doctor health --json",
                    "profile doctor capabilities --json",
                    "profile doctor robot-docs",
                    "profile doctor --robot-triage",
                    "profile doctor --fix"
                ],
                "status": "available"
            }
        },
        "registry_content_identity": {
            "field": "column_registry_hash",
            "algorithm": "blake3",
            "framing": "registry.json first, then sorted direct *.json mapping files excluding registry.json and _build.json; each frame is relative_path NUL byte_len NUL file_bytes 0xFF",
            "required_for": "frozen profiles that set column_registry",
            "drift_checked_by": ["validate", "lint", "stats", "slice"]
        },
        "canonical_csv_headers": {
            "canonicalizer_version": "profile.canonical_csv_headers.v1",
            "command": "profile slice",
            "enabled_when": "a resolved profile sets column_registry",
            "manifest_fields": ["input_hash", "profile_sha256", "column_registry_hash", "canonicalizer_version", "mapping", "unmapped"]
        },
        "commands": [
            {
                "name": "profile --robot-triage",
                "purpose": "return a machine-readable triage report without requiring --json",
                "reads_inputs": false,
                "writes_outputs": false
            },
            {
                "name": "profile capabilities --json",
                "purpose": "return the supported profile agent contract through the profile.v0 envelope",
                "reads_inputs": false,
                "writes_outputs": false
            },
            {
                "name": "profile robot-docs guide",
                "purpose": "print concise usage guidance for headless agents",
                "reads_inputs": false,
                "writes_outputs": false
            },
            {
                "name": "profile doctor health --json",
                "purpose": "return read-only health checks inside the profile.v0 envelope",
                "reads_inputs": false,
                "writes_outputs": false
            },
            {
                "name": "profile doctor capabilities --json",
                "purpose": "return the supported doctor contract and domain boundaries",
                "reads_inputs": false,
                "writes_outputs": false
            },
            {
                "name": "profile doctor robot-docs",
                "purpose": "print concise usage guidance for headless agents",
                "reads_inputs": false,
                "writes_outputs": false
            },
            {
                "name": "profile doctor --robot-triage",
                "purpose": "return a machine-readable triage report without requiring --json",
                "reads_inputs": false,
                "writes_outputs": false
            },
            {
                "name": "profile doctor --fix",
                "purpose": "refuse repair mode safely and name read-only alternatives",
                "reads_inputs": false,
                "writes_outputs": false
            }
        ],
        "fix_mode": {
            "available": false,
            "command": "profile doctor --fix",
            "behavior": "exits 2, emits only stderr, and names read-only alternatives",
            "reason": "No profile-specific fixer has detector, backup, inverse, and fixture coverage yet."
        },
        "composition": {
            "family": {
                "name": "cmdrvl-spine",
                "siblings": [
                    {"tool": "fingerprint", "capabilities": "fingerprint capabilities --json"},
                    {"tool": "canon", "capabilities": "canon doctor capabilities --json"},
                    {"tool": "profile", "capabilities": "profile capabilities --json"},
                    {"tool": "shape", "capabilities": "shape capabilities --json"},
                    {"tool": "rvl", "capabilities": "rvl capabilities --json"}
                ]
            },
            "role": "column-scope authoring and freezing before shape/rvl report tools",
            "position": "after fingerprint peek or canon normalization, before shape/rvl",
            "accepts": ["CSV dataset", "fingerprint peek JSON", "column registry", "draft profile YAML"],
            "produces": ["draft profile YAML", "frozen profile YAML", "sliced CSV"],
            "canonical_chain": [
                "fingerprint peek tape.csv --json --suggest > tape.peek.json",
                "profile draft init tape.csv --from-peek tape.peek.json --out loan_tape.draft.yaml",
                "profile freeze loan_tape.draft.yaml --family csv.loan_tape.core --version 0 --out profiles/csv.loan_tape.core.v0.yaml",
                "shape old.csv new.csv --profile profiles/csv.loan_tape.core.v0.yaml --json > evidence/shape.report.json",
                "rvl old.csv new.csv --profile profiles/csv.loan_tape.core.v0.yaml --json > evidence/rvl.report.json"
            ],
            "agent_rules": [
                "Use profile when shape/rvl should compare only an explicit column scope.",
                "Seed drafts from fingerprint peek when pre-parse or multi-row header hints exist.",
                "Freeze profiles before reusing them in repeatable evidence pipelines."
            ]
        },
        "detectors": detector_contracts(),
        "side_effects": side_effects(),
        "domain_boundaries": domain_boundaries()
    })
}

pub fn triage_report() -> Value {
    json!({
        "schema": "profile.doctor.triage.v1",
        "contract": CONTRACT,
        "status": "healthy",
        "healthy": true,
        "tool": tool_metadata(),
        "known_failure_modes": [
            {
                "id": "invalid_profile_schema",
                "classification": "refusal",
                "exit_code": 2,
                "operator_action": "run profile validate <FILE> --json and fix the profile YAML"
            },
            {
                "id": "dataset_column_mismatch",
                "classification": "domain_finding",
                "exit_code": 1,
                "operator_action": "run profile lint <PROFILE> --against <DATASET> --json"
            },
            {
                "id": "already_frozen_profile",
                "classification": "refusal",
                "exit_code": 2,
                "operator_action": "create a new profile version instead of mutating frozen YAML"
            },
            {
                "id": "witness_append_warning",
                "classification": "non_blocking_audit_warning",
                "exit_code": 0,
                "operator_action": "inspect EPISTEMIC_WITNESS or ~/.cmdrvl/state/witness/witness.jsonl permissions"
            },
            {
                "id": "remote_push_transport_failure",
                "classification": "refusal",
                "exit_code": 2,
                "operator_action": "run profile push <PROFILE> --json only after local validation passes"
            },
            {
                "id": "remote_pull_transport_failure",
                "classification": "refusal",
                "exit_code": 2,
                "operator_action": "run profile pull <PROFILE_ID> --out <DIR> --json and inspect transport refusal details"
            }
        ],
        "detectors": detector_contracts(),
        "recommended_actions": [
            {
                "priority": 1,
                "action": "profile capabilities --json",
                "reason": "discover the supported read-only diagnostic contract from the top-level agent surface"
            },
            {
                "priority": 2,
                "action": "profile validate <PROFILE> --json --no-witness",
                "reason": "validate a specific profile only after an explicit profile path is available"
            },
            {
                "priority": 3,
                "action": "profile lint <PROFILE> --against <DATASET> --json --no-witness",
                "reason": "check profile-to-dataset alignment only after both paths are explicit"
            }
        ],
        "side_effects": side_effects()
    })
}

fn detector_contracts() -> Value {
    json!([
        {
            "id": "invalid_profile_schema",
            "fixture": "tests/fixtures/profiles/invalid/frozen_bad_sha.yaml",
            "command": "profile validate <FILE> --json --no-witness",
            "fixer_allowed": false
        },
        {
            "id": "dataset_column_mismatch",
            "fixture": "tests/fixtures/datasets/valid/loan_tape_missing_rate.csv",
            "command": "profile lint <PROFILE> --against <DATASET> --json --no-witness",
            "fixer_allowed": false
        },
        {
            "id": "already_frozen_profile",
            "fixture": "tests/fixtures/profiles/valid/frozen_complete.yaml",
            "command": "profile freeze <FROZEN_PROFILE> --family <FAMILY> --version <N> --out <OUT> --json --no-witness",
            "fixer_allowed": false
        },
        {
            "id": "witness_append_warning",
            "fixture": null,
            "command": "profile validate <PROFILE> --json",
            "fixer_allowed": false
        },
        {
            "id": "remote_push_transport_failure",
            "fixture": null,
            "command": "profile push <PROFILE> --json --no-witness",
            "fixer_allowed": false
        },
        {
            "id": "remote_pull_transport_failure",
            "fixture": null,
            "command": "profile pull <PROFILE_ID> --out <DIR> --json --no-witness",
            "fixer_allowed": false
        }
    ])
}

pub fn robot_docs() -> Value {
    json!({
        "schema": "profile.doctor.robot_docs.v1",
        "contract": CONTRACT,
        "text": ROBOT_DOCS
    })
}

const ROBOT_DOCS: &str = r#"profile doctor is read-only.

Use:
- profile --robot-triage
- profile capabilities --json
- profile robot-docs guide
- profile doctor health --json
- profile doctor capabilities --json
- profile doctor --robot-triage
- profile doctor robot-docs
- profile doctor --fix

The doctor does not read profile files, datasets, column registries, stdin, witness ledgers, or network endpoints. It does not write profile YAML, witness records, .doctor artifacts, or remote data.

Composition:
- fingerprint peek tape.csv --json --suggest > tape.peek.json
- profile draft init tape.csv --from-peek tape.peek.json --out loan_tape.draft.yaml
- profile freeze loan_tape.draft.yaml --family csv.loan_tape.core --version 0 --out profiles/csv.loan_tape.core.v0.yaml
- shape old.csv new.csv --profile profiles/csv.loan_tape.core.v0.yaml --json > evidence/shape.report.json
- rvl old.csv new.csv --profile profiles/csv.loan_tape.core.v0.yaml --json > evidence/rvl.report.json

Do not use profile doctor as a replacement for validation or linting. Once explicit paths are known, use profile validate <FILE> --json --no-witness or profile lint <PROFILE> --against <DATASET> --json --no-witness.

profile doctor --fix is unavailable and exits 2 without stdout. Profile repair must remain manual until each fixer has detector, backup, inverse, and fixture coverage."#;

// The v1 surfaces below are the canonical machine-facing contract. The older
// report helpers above remain available to the existing human-oriented docs
// command while callers migrate to these direct JSON reports.
const DOCTOR_SCHEMA_VERSION: &str = "cmdrvl.doctor.v1";
const OPERATOR_MANIFEST: &str = include_str!("../operator.json");

pub fn handle_command(args: Option<&DoctorArgs>, top_level_triage: bool, json_output: bool) -> u8 {
    if top_level_triage {
        return emit_triage(true);
    }

    let Some(args) = args else {
        return emit_health(json_output);
    };
    if args.fix {
        return emit_fix_unavailable();
    }
    if args.robot_triage {
        return emit_triage(true);
    }

    match args.command.as_ref() {
        Some(DoctorCommand::Capabilities) => emit_doctor_capabilities(json_output),
        Some(DoctorCommand::Triage(triage)) => emit_triage(json_output || triage.json),
        Some(DoctorCommand::RobotDocs) => {
            println!("{ROBOT_DOCS_V1}");
            0
        }
        Some(DoctorCommand::Fix) => emit_fix_unavailable(),
        Some(DoctorCommand::Health) | None => emit_health(json_output),
    }
}

pub fn emit_capabilities(json_output: bool) -> u8 {
    if json_output {
        print_operator_json_v1();
    } else {
        println!("profile capabilities");
        println!("operator contract: profile --describe");
        println!("doctor: profile doctor --json");
        println!("triage: profile doctor triage --json");
        println!("robot docs: profile robot-docs guide");
    }
    0
}

pub fn emit_operator_json() -> u8 {
    print_operator_json_v1();
    0
}

fn emit_health(json_output: bool) -> u8 {
    let report = v1_doctor_report("health");
    if json_output {
        print_json_v1(&report);
    } else {
        print_health_text_v1(&report);
    }
    v1_doctor_exit_code(&report)
}

fn emit_doctor_capabilities(json_output: bool) -> u8 {
    let report = v1_doctor_report("capabilities");
    if json_output {
        print_json_v1(&report);
    } else {
        println!("profile doctor capabilities");
        println!("contract: profile capabilities --json");
        println!("healthy: {}", report["healthy"]);
    }
    v1_doctor_exit_code(&report)
}

fn emit_triage(json_output: bool) -> u8 {
    let report = v1_doctor_report("triage");
    if json_output {
        print_json_v1(&report);
    } else {
        print_health_text_v1(&report);
        println!("next: profile capabilities --json");
    }
    v1_doctor_exit_code(&report)
}

fn v1_doctor_report(mode: &str) -> Value {
    let findings = v1_health_findings();
    let healthy = !findings
        .iter()
        .any(|finding| finding.get("severity").and_then(Value::as_str) == Some("error"));
    let mut report = json!({
        "schema_version": DOCTOR_SCHEMA_VERSION,
        "mode": mode,
        "tool": "profile",
        "tool_version": env!("CARGO_PKG_VERSION"),
        "healthy": healthy,
        "findings": findings,
    });

    match mode {
        "health" => {}
        "capabilities" => report["contract"] = v1_operator_contract(),
        "triage" => {
            report["next_actions"] = if healthy {
                json!(["profile capabilities --json", "profile robot-docs guide"])
            } else {
                json!([
                    "profile doctor --json",
                    "profile doctor capabilities --json"
                ])
            };
        }
        _ => unreachable!("doctor mode is fixed by the caller"),
    }
    report
}

fn v1_health_findings() -> Vec<Value> {
    let manifest_ok = v1_operator_manifest_is_valid();
    let epoch_ok = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .is_none_or(|value| value.parse::<i64>().is_ok());
    // Reports only whether a usable key is configured: never its value or length.
    let (redaction_severity, redaction_message) = match crate::redaction::RedactionKey::from_env() {
        Ok(Some(_)) => ("info", "redaction key: configured (keyed markers)"),
        Ok(None) => ("info", "redaction key: not configured (ordinal refs)"),
        Err(_) => (
            "error",
            "redaction key: set but unusable, so slice, inspect, stats and suggest-key refuse (E_REDACTION_KEY_INVALID)",
        ),
    };
    vec![
        json!({
            "id": "profile.operator_manifest",
            "severity": if manifest_ok { "info" } else { "error" },
            "message": if manifest_ok {
                "operator.v1 manifest is valid and exposes capabilities, robot-docs, and doctor"
            } else {
                "operator.v1 manifest is invalid"
            },
            "fix": if manifest_ok { Value::Null } else {
                json!("inspect profile --describe")
            }
        }),
        json!({
            "id": "profile.redaction_key",
            "severity": redaction_severity,
            "message": redaction_message,
        }),
        json!({
            "id": "profile.doctor_read_only",
            "severity": "info",
            "message": "doctor returns before profile, dataset, registry, witness, or network handlers"
        }),
        json!({
            "id": "profile.source_date_epoch",
            "severity": if epoch_ok { "info" } else { "warning" },
            "message": if epoch_ok {
                "witness timestamps honor SOURCE_DATE_EPOCH when it is a valid Unix timestamp"
            } else {
                "SOURCE_DATE_EPOCH is not an integer Unix timestamp"
            },
            "fix": if epoch_ok { Value::Null } else {
                json!("set SOURCE_DATE_EPOCH to an integer Unix timestamp")
            }
        }),
    ]
    .into_iter()
    .map(|mut finding| {
        if finding["fix"].is_null() {
            finding
                .as_object_mut()
                .expect("finding is an object")
                .remove("fix");
        }
        finding
    })
    .collect()
}

fn v1_operator_contract() -> Value {
    serde_json::from_str(OPERATOR_MANIFEST).expect("operator.json must be valid JSON")
}

fn v1_operator_manifest_is_valid() -> bool {
    let Ok(value) = serde_json::from_str::<Value>(OPERATOR_MANIFEST) else {
        return false;
    };
    let commands_ok = ["capabilities", "robot-docs", "doctor"]
        .into_iter()
        .all(|expected| {
            value
                .get("commands")
                .and_then(Value::as_array)
                .is_some_and(|commands| {
                    commands.iter().any(|command| {
                        command.get("name").and_then(Value::as_str) == Some(expected)
                    })
                })
        });
    value.get("schema_version").and_then(Value::as_str) == Some("operator.v1")
        && value.get("name").and_then(Value::as_str) == Some("profile")
        && value
            .get("version")
            .or_else(|| value.get("tool_version"))
            .and_then(Value::as_str)
            == Some(env!("CARGO_PKG_VERSION"))
        && value.get("contract_version").and_then(Value::as_str) == Some("1.0.0")
        && commands_ok
}

fn v1_doctor_exit_code(report: &Value) -> u8 {
    if report["healthy"].as_bool() == Some(true) {
        0
    } else {
        1
    }
}

fn print_operator_json_v1() {
    print!("{OPERATOR_MANIFEST}");
    if !OPERATOR_MANIFEST.ends_with('\n') {
        println!();
    }
}

fn print_json_v1(value: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
    );
}

fn print_health_text_v1(report: &Value) {
    let healthy = report["healthy"].as_bool().unwrap_or(false);
    println!(
        "profile doctor: {}",
        if healthy { "healthy" } else { "unhealthy" }
    );
    if let Some(findings) = report.get("findings").and_then(Value::as_array) {
        for finding in findings {
            println!(
                "- {}: {}",
                finding["id"].as_str().unwrap_or("unknown"),
                finding["message"].as_str().unwrap_or("unknown")
            );
        }
    }
}

const ROBOT_DOCS_V1: &str = r#"profile robot-docs guide

Discovery:
- profile --robot-triage
- profile capabilities --json
- profile robot-docs guide
- profile doctor --json
- profile doctor health --json (legacy alias)
- profile doctor capabilities --json
- profile doctor triage --json
- profile doctor --robot-triage (legacy alias)

Flags:
- Use --out as the canonical output flag. --output remains a compatibility alias.
- Use --json for machine-readable reports.
- Set NO_COLOR, TERM=dumb, or use a non-TTY to keep output ANSI-free.
- Set SOURCE_DATE_EPOCH to an integer Unix timestamp for reproducible witness timestamps.

The doctor is read-only. It does not read profile files, datasets, registries, stdin, or witness ledgers, and it does not write profile YAML, witness records, or remote data.

Composition:
- fingerprint peek tape.csv --json --suggest > tape.peek.json
- profile draft init tape.csv --from-peek tape.peek.json --out loan_tape.draft.yaml
- profile freeze loan_tape.draft.yaml --family csv.loan_tape.core --version 0 --out profiles/csv.loan_tape.core.v0.yaml
- shape old.csv new.csv --profile profiles/csv.loan_tape.core.v0.yaml --json > evidence/shape.report.json
- rvl old.csv new.csv --profile profiles/csv.loan_tape.core.v0.yaml --json > evidence/rvl.report.json

profile doctor --fix is unavailable and exits 2."#;
