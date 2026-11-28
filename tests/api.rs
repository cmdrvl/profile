use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use profile::api::{
    BytesOptions, draft_bytes, evolve, inspect_bytes, inspect_bytes_with_options, slice_bytes,
    suggest_key_bytes,
};
use profile::redaction::RedactionKey;
use profile::schema::{Profile, ProfileStatus, canonical_yaml, compute_profile_sha256};
use serde_json::Value;

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(relative)
}

fn cli_result(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_profile"))
        .args(args)
        .output()
        .expect("profile CLI should start");
    assert!(
        output.status.success(),
        "CLI failed: status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("CLI should emit JSON")
}

#[test]
fn inspect_bytes_matches_cli_result_for_default_and_explicit_redaction() {
    let path = fixture("datasets/valid/loan_tape_basic.csv");
    let bytes = fs::read(&path).expect("fixture should be readable");
    for explicit in [false, true] {
        let api = inspect_bytes(&bytes, explicit).expect("in-memory inspect should succeed");
        let args = if explicit {
            vec![
                "--json",
                "--no-witness",
                "--explicit",
                "inspect",
                path.to_str().unwrap(),
            ]
        } else {
            vec!["--json", "--no-witness", "inspect", path.to_str().unwrap()]
        };
        assert_eq!(api, cli_result(&args)["result"]);
    }
}

#[test]
fn suggest_key_bytes_matches_cli_result_for_default_and_explicit_redaction() {
    let path = fixture("datasets/valid/loan_tape_basic.csv");
    let bytes = fs::read(&path).expect("fixture should be readable");
    for explicit in [false, true] {
        let api =
            suggest_key_bytes(&bytes, 5, 1, explicit).expect("in-memory suggest should succeed");
        let args = if explicit {
            vec![
                "--json",
                "--no-witness",
                "--explicit",
                "suggest-key",
                path.to_str().unwrap(),
            ]
        } else {
            vec![
                "--json",
                "--no-witness",
                "suggest-key",
                path.to_str().unwrap(),
            ]
        };
        assert_eq!(api, cli_result(&args)["result"]);
    }
}

#[test]
fn draft_bytes_is_a_valid_profile_and_evolve_returns_a_mutable_draft() {
    let source = b"loan_id,balance\nA,10\nB,20\n";
    let draft = draft_bytes(source).expect("in-memory draft should succeed");
    assert_eq!(draft.status, ProfileStatus::Draft);
    assert_eq!(draft.include_columns, ["loan_id", "balance"]);
    assert_eq!(draft.columns["balance"].type_.unwrap().as_str(), "integer");

    let mut frozen = draft.clone();
    frozen.status = ProfileStatus::Frozen;
    frozen.profile_id = Some("csv.example.v0".to_owned());
    frozen.profile_family = Some("csv.example".to_owned());
    frozen.profile_version = Some(0);
    frozen.profile_sha256 = Some(compute_profile_sha256(
        &canonical_yaml(&frozen).expect("frozen profile should canonicalize"),
    ));
    let evolved = evolve(&frozen, b"loan_id,current_balance\nA,10\nB,20\n")
        .expect("in-memory evolve should succeed");
    assert_eq!(evolved.status, ProfileStatus::Draft);
    assert!(evolved.profile_id.is_none());
    assert_eq!(evolved.include_columns, ["loan_id", "current_balance"]);
}

#[test]
fn slice_bytes_preserves_csv_bytes_and_redacts_headers_by_default() {
    let output = slice_bytes(b"banner\nloan_id,balance\nA,10\n", 1, false)
        .expect("in-memory slice should succeed");
    assert_eq!(output.csv, b"loan_id,balance\nA,10\n");
    assert_eq!(
        output.result["columns"],
        serde_json::json!(["redacted:ref:c1", "redacted:ref:c2"])
    );
    assert_eq!(output.result["redaction"], "ordinal");

    let explicit = slice_bytes(b"banner\nloan_id,balance\nA,10\n", 1, true)
        .expect("explicit in-memory slice should succeed");
    assert_eq!(
        explicit.result["columns"],
        serde_json::json!(["loan_id", "balance"])
    );
    assert_eq!(explicit.result["slice_csv"], "loan_id,balance\nA,10\n");
}

#[test]
fn keyed_redaction_is_explicit_and_does_not_read_the_environment() {
    let key = RedactionKey::from_env_value(Some(OsString::from("0123456789abcdef")))
        .expect("test key should validate")
        .expect("test key should be present");
    let options = BytesOptions {
        redaction_key: Some(key),
        ..BytesOptions::default()
    };
    let output = inspect_bytes_with_options(b"loan_id,balance\nA,10\n", &options)
        .expect("keyed inspect should succeed");
    assert_eq!(output["redaction"], "keyed");
    assert!(
        output["columns"][0]["id"]
            .as_str()
            .is_some_and(|value| value.starts_with("redacted:hmac-sha256:"))
    );
}

#[test]
fn api_refusal_does_not_read_or_write_the_filesystem() {
    let error = inspect_bytes(b"header-only\n", false).expect_err("missing data must refuse");
    assert_eq!(error.code, "E_EMPTY");
    let error = draft_bytes(b"not,csv\n\"unterminated\n").expect_err("bad CSV must refuse");
    assert_eq!(error.code, "E_CSV_PARSE");
    let options = BytesOptions {
        name_hint: Some("input.csv".to_owned()),
        ..BytesOptions::default()
    };
    assert!(inspect_bytes(&[], false).is_err());
    assert!(profile::api::inspect_bytes_with_options(b"a\n1\n", &options).is_ok());
}

#[test]
fn api_profile_yaml_round_trips_without_cli_state() {
    let profile = draft_bytes(b"loan_id,balance\nA,10\nB,20\n").expect("draft should succeed");
    let yaml = profile.to_yaml().expect("profile YAML should serialize");
    let round_trip: Profile = Profile::from_yaml(&yaml).expect("profile YAML should parse");
    assert_eq!(round_trip, profile);
}
