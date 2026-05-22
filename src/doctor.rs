use serde_json::{Value, json};

use crate::cli::args::{DoctorArgs, DoctorCommand};
use crate::refusal::RefusalPayload;

const CONTRACT: &str = "cmdrvl.read_only_doctor.v1";
const TOOL_ROLE: &str = "configuration authoring tool for deterministic column-scoping profiles";

pub fn run(args: &DoctorArgs) -> Result<Value, RefusalPayload> {
    if args.robot_triage {
        return Ok(triage_report());
    }

    match args.command.as_ref().unwrap_or(&DoctorCommand::Health) {
        DoctorCommand::Health => Ok(health_report()),
        DoctorCommand::Capabilities => Ok(capabilities_report()),
        DoctorCommand::RobotDocs => Ok(robot_docs()),
    }
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
                "detail": "doctor --fix is not part of the clap surface"
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

fn capabilities_report() -> Value {
    json!({
        "schema": "profile.doctor.capabilities.v1",
        "contract": CONTRACT,
        "status": "available",
        "tool": tool_metadata(),
        "commands": [
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
            }
        ],
        "fix_mode": {
            "available": false,
            "reason": "No profile-specific fixer has detector, backup, inverse, and fixture coverage yet."
        },
        "detectors": detector_contracts(),
        "side_effects": side_effects(),
        "domain_boundaries": domain_boundaries()
    })
}

fn triage_report() -> Value {
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
                "action": "profile doctor capabilities --json",
                "reason": "discover the supported read-only diagnostic contract"
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
            "fixture": "tests/fixtures/profiles/invalid_schema.yaml",
            "command": "profile validate <FILE> --json --no-witness",
            "fixer_allowed": false
        },
        {
            "id": "dataset_column_mismatch",
            "fixture": "tests/fixtures/datasets/missing_columns.csv",
            "command": "profile lint <PROFILE> --against <DATASET> --json --no-witness",
            "fixer_allowed": false
        },
        {
            "id": "already_frozen_profile",
            "fixture": "tests/fixtures/profiles/frozen_valid.yaml",
            "command": "profile freeze <FROZEN_PROFILE> --family <FAMILY> --version <N> --out <OUT> --json --no-witness",
            "fixer_allowed": false
        },
        {
            "id": "witness_append_warning",
            "fixture": "tests/fixtures/witness/unwritable-ledger",
            "command": "profile validate <PROFILE> --json",
            "fixer_allowed": false
        },
        {
            "id": "remote_push_transport_failure",
            "fixture": "tests/network_push_transport_failure",
            "command": "profile push <PROFILE> --json --no-witness",
            "fixer_allowed": false
        },
        {
            "id": "remote_pull_transport_failure",
            "fixture": "tests/network_pull_transport_failure",
            "command": "profile pull <PROFILE_ID> --out <DIR> --json --no-witness",
            "fixer_allowed": false
        }
    ])
}

fn robot_docs() -> Value {
    json!({
        "schema": "profile.doctor.robot_docs.v1",
        "contract": CONTRACT,
        "text": ROBOT_DOCS
    })
}

const ROBOT_DOCS: &str = r#"profile doctor is read-only.

Use:
- profile doctor health --json
- profile doctor capabilities --json
- profile doctor --robot-triage
- profile doctor robot-docs

The doctor does not read profile files, datasets, column registries, stdin, witness ledgers, or network endpoints. It does not write profile YAML, witness records, .doctor artifacts, or remote data.

Do not use profile doctor as a replacement for validation or linting. Once explicit paths are known, use profile validate <FILE> --json --no-witness or profile lint <PROFILE> --against <DATASET> --json --no-witness.

There is no doctor --fix mode. Profile repair must remain manual until each fixer has detector, backup, inverse, and fixture coverage."#;
