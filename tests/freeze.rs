mod common;

use std::fs;

use common::{
    assert_json_envelope_shape, fixture_path, parse_stdout_json, profile_cmd, temp_workspace,
};
use profile::schema::{Profile, canonical_yaml, compute_profile_sha256, registry_content_hash};
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
fn freeze_registry_free_sha256_regression_stays_unchanged() {
    let workspace = temp_workspace();
    let out_path = workspace.path().join("frozen.yaml");

    let assert = profile_cmd()
        .arg("--no-witness")
        .arg("freeze")
        .arg(fixture_path("profiles/valid/draft_with_key.yaml"))
        .arg("--family")
        .arg("csv.loan_tape.core")
        .arg("--version")
        .arg("0")
        .arg("--out")
        .arg(&out_path)
        .assert();
    common::assert_success_exit!(assert);

    let frozen = Profile::from_yaml(
        &fs::read_to_string(&out_path).expect("frozen profile should be readable"),
    )
    .expect("frozen profile should parse");
    assert_eq!(
        frozen.profile_sha256.as_deref(),
        Some("sha256:79dfeeb23cda6d894d756c84e7aca1b244dd7a8ab4ed24aed44908589635e5bf")
    );
    assert_eq!(frozen.column_registry_hash, None);
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

#[test]
fn freeze_preserves_column_registry_when_present() {
    let workspace = temp_workspace();
    let registry_dir = workspace.path().join("registries").join("annex_columns_v0");
    common::copy_fixture(
        "registries/annex_columns_v0/registry.json",
        registry_dir.join("registry.json"),
    );
    common::copy_fixture(
        "registries/annex_columns_v0/aliases.json",
        registry_dir.join("aliases.json"),
    );
    let draft_path = workspace.path().join("draft.yaml");
    let out_path = workspace.path().join("frozen.yaml");
    fs::write(
        &draft_path,
        "\
schema_version: 1
status: draft
format: csv
column_registry: registries/annex_columns_v0
equivalence:
  float_decimals: 6
  trim_strings: true
key:
  - loan_id_number
include_columns:
  - loan_id_number
  - current_balance
",
    )
    .expect("draft fixture write should succeed");

    let assert = profile_cmd()
        .arg("--no-witness")
        .arg("freeze")
        .arg(&draft_path)
        .arg("--family")
        .arg("csv.loan_tape.registry")
        .arg("--version")
        .arg("0")
        .arg("--out")
        .arg(&out_path)
        .assert();
    common::assert_success_exit!(assert);

    let content = fs::read_to_string(&out_path).expect("frozen profile should be readable");
    assert!(
        content.contains("column_registry: registries/annex_columns_v0"),
        "expected frozen profile to retain column_registry field"
    );
}

#[test]
fn freeze_writes_column_registry_hash_and_hashes_it_into_profile_sha256() {
    let workspace = temp_workspace();
    let registry_dir = workspace.path().join("registries").join("annex_columns_v0");
    common::copy_fixture(
        "registries/annex_columns_v0/registry.json",
        registry_dir.join("registry.json"),
    );
    common::copy_fixture(
        "registries/annex_columns_v0/aliases.json",
        registry_dir.join("aliases.json"),
    );
    let expected_registry_hash =
        registry_content_hash(&registry_dir).expect("registry should hash");

    let draft_path = workspace.path().join("draft.yaml");
    let out_path = workspace.path().join("frozen.yaml");
    fs::write(
        &draft_path,
        "\
schema_version: 1
status: draft
format: csv
column_registry: registries/annex_columns_v0
equivalence:
  float_decimals: 6
  trim_strings: true
key:
  - loan_id_number
include_columns:
  - loan_id_number
  - current_balance
",
    )
    .expect("draft fixture write should succeed");

    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("freeze")
        .arg(&draft_path)
        .arg("--family")
        .arg("csv.loan_tape.registry")
        .arg("--version")
        .arg("0")
        .arg("--out")
        .arg(&out_path)
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert_eq!(
        envelope["result"]["column_registry_hash"].as_str(),
        Some(expected_registry_hash.as_str())
    );

    let content = fs::read_to_string(&out_path).expect("frozen profile should be readable");
    assert!(content.contains(&format!("column_registry_hash: {expected_registry_hash}\n")));
    assert!(
        content
            .find("column_registry:")
            .expect("column_registry line")
            < content
                .find("column_registry_hash:")
                .expect("column_registry_hash line")
    );

    let frozen = Profile::from_yaml(&content).expect("frozen profile should parse");
    assert_eq!(
        frozen.column_registry_hash.as_deref(),
        Some(expected_registry_hash.as_str())
    );
    let canonical = canonical_yaml(&frozen).expect("frozen profile should canonicalize");
    assert!(canonical.contains(&format!("column_registry_hash: {expected_registry_hash}\n")));
    let expected_profile_sha = compute_profile_sha256(&canonical);
    assert_eq!(
        frozen.profile_sha256.as_deref(),
        Some(expected_profile_sha.as_str())
    );
}

#[test]
fn column_registry_hash_is_independent_of_absolute_or_relative_locator() {
    let workspace = temp_workspace();
    let registry_dir = workspace.path().join("registries").join("annex_columns_v0");
    common::copy_fixture(
        "registries/annex_columns_v0/registry.json",
        registry_dir.join("registry.json"),
    );
    common::copy_fixture(
        "registries/annex_columns_v0/aliases.json",
        registry_dir.join("aliases.json"),
    );

    let relative_draft = workspace.path().join("relative.yaml");
    fs::write(
        &relative_draft,
        "\
schema_version: 1
status: draft
format: csv
column_registry: registries/annex_columns_v0
include_columns:
  - loan_id_number
",
    )
    .expect("relative draft should be written");

    let absolute_draft = workspace.path().join("absolute.yaml");
    fs::write(
        &absolute_draft,
        format!(
            "\
schema_version: 1
status: draft
format: csv
column_registry: {}
include_columns:
  - loan_id_number
",
            registry_dir.display()
        ),
    )
    .expect("absolute draft should be written");

    let relative_out = workspace.path().join("relative-frozen.yaml");
    let absolute_out = workspace.path().join("absolute-frozen.yaml");
    for (draft, out) in [
        (relative_draft.as_path(), relative_out.as_path()),
        (absolute_draft.as_path(), absolute_out.as_path()),
    ] {
        let assert = profile_cmd()
            .arg("--no-witness")
            .arg("freeze")
            .arg(draft)
            .arg("--family")
            .arg("csv.loan_tape.registry")
            .arg("--version")
            .arg("0")
            .arg("--out")
            .arg(out)
            .assert();
        common::assert_success_exit!(assert);
    }

    let relative = Profile::from_yaml(
        &fs::read_to_string(&relative_out).expect("relative frozen profile should be readable"),
    )
    .expect("relative frozen profile should parse");
    let absolute = Profile::from_yaml(
        &fs::read_to_string(&absolute_out).expect("absolute frozen profile should be readable"),
    )
    .expect("absolute frozen profile should parse");

    assert_eq!(relative.column_registry_hash, absolute.column_registry_hash);
    assert_ne!(relative.profile_sha256, absolute.profile_sha256);
}

#[test]
fn registry_content_hash_is_stable_ordered_and_sensitive_to_semantic_files() {
    let workspace = temp_workspace();
    let left = workspace.path().join("left");
    let right = workspace.path().join("right");
    fs::create_dir_all(&left).expect("left registry should be created");
    fs::create_dir_all(&right).expect("right registry should be created");

    write_registry_files_in_reverse_order(&left);
    write_registry_files_in_reverse_order(&right);

    let left_hash = registry_content_hash(&left).expect("left registry should hash");
    let right_hash = registry_content_hash(&right).expect("right registry should hash");
    assert_eq!(left_hash, right_hash);

    fs::write(left.join("_build.json"), "{\"generated\":true}\n")
        .expect("_build metadata should be written");
    assert_eq!(
        registry_content_hash(&left).expect("registry should hash after _build change"),
        left_hash
    );

    fs::write(
        left.join("a.json"),
        "[{\"input\":\"A\",\"canonical_id\":\"a_changed\",\"canonical_type\":\"column_name\",\"rule_id\":\"A\"}]\n",
    )
    .expect("semantic mapping file should be mutated");
    assert_ne!(
        registry_content_hash(&left).expect("mutated registry should hash"),
        left_hash
    );
}

#[test]
fn frozen_registry_hash_drift_refuses_validate_lint_stats_and_slice() {
    let workspace = temp_workspace();
    let registry_dir = workspace.path().join("registries").join("annex_columns_v0");
    common::copy_fixture(
        "registries/annex_columns_v0/registry.json",
        registry_dir.join("registry.json"),
    );
    common::copy_fixture(
        "registries/annex_columns_v0/aliases.json",
        registry_dir.join("aliases.json"),
    );

    let draft_path = workspace.path().join("draft.yaml");
    let profile_path = workspace.path().join("frozen.yaml");
    fs::write(
        &draft_path,
        "\
schema_version: 1
status: draft
format: csv
column_registry: registries/annex_columns_v0
pre_parse:
  slice:
    mode: preamble_skip
    header_at_row: 1
    data_starts_at: 2
key:
  - loan_id_number
include_columns:
  - loan_id_number
  - current_balance
",
    )
    .expect("draft fixture write should succeed");

    let freeze = profile_cmd()
        .arg("--no-witness")
        .arg("freeze")
        .arg(&draft_path)
        .arg("--family")
        .arg("csv.loan_tape.registry")
        .arg("--version")
        .arg("0")
        .arg("--out")
        .arg(&profile_path)
        .assert();
    common::assert_success_exit!(freeze);

    let aliases_path = registry_dir.join("aliases.json");
    let aliases = fs::read_to_string(&aliases_path).expect("aliases should be readable");
    fs::write(
        &aliases_path,
        aliases.replacen("loan_id_number", "loan_id_number_changed", 1),
    )
    .expect("aliases should be mutated");

    assert_registry_drift_refusal(
        profile_cmd()
            .arg("--json")
            .arg("--no-witness")
            .arg("validate")
            .arg(&profile_path)
            .assert(),
    );
    assert_registry_drift_refusal(
        profile_cmd()
            .arg("--json")
            .arg("--no-witness")
            .arg("lint")
            .arg(&profile_path)
            .arg("--against")
            .arg(fixture_path("datasets/valid/loan_tape_alt_headers.csv"))
            .assert(),
    );
    assert_registry_drift_refusal(
        profile_cmd()
            .arg("--json")
            .arg("--no-witness")
            .arg("stats")
            .arg(fixture_path("datasets/valid/loan_tape_alt_headers.csv"))
            .arg("--profile")
            .arg(&profile_path)
            .assert(),
    );
    assert_registry_drift_refusal(
        profile_cmd()
            .arg("--json")
            .arg("--no-witness")
            .arg("slice")
            .arg(fixture_path("datasets/valid/loan_tape_alt_headers.csv"))
            .arg("--profile-path")
            .arg(&profile_path)
            .assert(),
    );
}

#[test]
fn freeze_preserves_composite_key_order_and_hashes_order_as_identity() {
    let workspace = temp_workspace();
    let ordered_out = workspace.path().join("ordered.yaml");
    let reordered_draft = workspace.path().join("reordered-draft.yaml");
    let reordered_out = workspace.path().join("reordered.yaml");

    fs::write(
        &reordered_draft,
        "schema_version: 1\nstatus: draft\nformat: csv\nkey:\n  - loan_id\n  - property_type\ninclude_columns:\n  - loan_id\n  - balance\n  - rate\n  - property_type\nequivalence:\n  float_decimals: 6\n  trim_strings: true\n",
    )
    .expect("reordered draft should be written");

    for (draft, output) in [
        (
            fixture_path("profiles/valid/draft_composite_key.yaml"),
            ordered_out.as_path(),
        ),
        (reordered_draft.clone(), reordered_out.as_path()),
    ] {
        let assert = profile_cmd()
            .arg("--no-witness")
            .arg("freeze")
            .arg(draft)
            .arg("--family")
            .arg("csv.loan_tape.composite")
            .arg("--version")
            .arg("0")
            .arg("--out")
            .arg(output)
            .assert();
        common::assert_success_exit!(assert);
    }

    let ordered = Profile::from_yaml(
        &fs::read_to_string(&ordered_out).expect("ordered frozen profile should be readable"),
    )
    .expect("ordered frozen profile should parse");
    let reordered = Profile::from_yaml(
        &fs::read_to_string(&reordered_out).expect("reordered frozen profile should be readable"),
    )
    .expect("reordered frozen profile should parse");

    assert_eq!(ordered.key, ["property_type", "loan_id"]);
    assert_eq!(reordered.key, ["loan_id", "property_type"]);

    for frozen in [&ordered, &reordered] {
        let canonical = canonical_yaml(frozen).expect("frozen profile should canonicalize");
        let expected_sha = compute_profile_sha256(&canonical);
        assert_eq!(
            frozen.profile_sha256.as_deref(),
            Some(expected_sha.as_str())
        );

        let round_trip = Profile::from_yaml(
            &frozen
                .to_yaml()
                .expect("frozen profile should serialize for round trip"),
        )
        .expect("round-tripped profile should parse");
        assert_eq!(round_trip.key, frozen.key);
    }

    assert_ne!(ordered.profile_sha256, reordered.profile_sha256);
}

fn write_registry_files_in_reverse_order(registry_dir: &std::path::Path) {
    fs::write(
        registry_dir.join("registry.json"),
        "{\"id\":\"fixture\",\"version\":\"1\"}\n",
    )
    .expect("registry.json should be written");
    fs::write(
        registry_dir.join("z.json"),
        "[{\"input\":\"Z\",\"canonical_id\":\"z\",\"canonical_type\":\"column_name\",\"rule_id\":\"Z\"}]\n",
    )
    .expect("z mapping should be written");
    fs::write(
        registry_dir.join("a.json"),
        "[{\"input\":\"A\",\"canonical_id\":\"a\",\"canonical_type\":\"column_name\",\"rule_id\":\"A\"}]\n",
    )
    .expect("a mapping should be written");
}

fn assert_registry_drift_refusal(assert: assert_cmd::assert::Assert) {
    let envelope = parse_stdout_json(&assert);
    common::assert_refusal_exit!(assert);
    assert_eq!(envelope["result"]["code"], "E_INVALID_SCHEMA");
    assert_eq!(
        envelope["result"]["detail"]["errors"][0]["field"],
        "column_registry_hash"
    );
}
