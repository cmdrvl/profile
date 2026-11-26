//! bd-bix: `profile evolve` never prints dataset header text without --explicit. A new column is
//! an ordinal ref in header-row order (or a keyed HMAC marker); profile-side names stay.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::temp_workspace;
use serde_json::Value;

const KEY: &str = "0123456789abcdef";
const OTHER_KEY: &str = "fedcba9876543210";
const KEY_ENV: &str = "PROFILE_REDACTION_KEY";
/// A header that is new to the profile, and one that renames the profile's key column.
const NEW_COLUMN: &str = "NEWCOL-Zq9x-7f2e";
const RENAMED_KEY: &str = "Canary-Loan-ID";
const CANARY_HEADERS: &str = "Canary-Loan-ID,canary_balance,NEWCOL-Zq9x-7f2e";

fn write_frozen_profile(path: &Path) {
    fs::write(
        path,
        format!(
            "schema_version: 1\nprofile_id: csv.canary.v0\nprofile_version: 0\nprofile_family: csv.canary\nprofile_sha256: sha256:{:0<64}\nstatus: frozen\nformat: csv\nkey:\n  - canary_loan_id\ninclude_columns:\n  - canary_loan_id\n  - canary_balance\n",
            "1"
        ),
    )
    .unwrap();
}

struct Fixture {
    _dir: tempfile::TempDir,
    profile: PathBuf,
    dataset: PathBuf,
    draft: PathBuf,
}

fn fixture(headers: &str) -> Fixture {
    let dir = temp_workspace();
    let profile = dir.path().join("old.yaml");
    write_frozen_profile(&profile);
    let dataset = dir.path().join("new.csv");
    fs::write(&dataset, format!("{headers}\nA,10,x\nB,20,y\n")).unwrap();
    let draft = dir.path().join("next.draft.yaml");
    Fixture {
        profile,
        dataset,
        draft,
        _dir: dir,
    }
}

/// `profile --no-witness <global flags> evolve ... <evolve flags>`.
fn evolve(f: &Fixture, global: &[&str], evolve_flags: &[&str], key: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_profile"));
    command.env_remove(KEY_ENV);
    if let Some(key) = key {
        command.env(KEY_ENV, key);
    }
    command
        .arg("--no-witness")
        .args(global)
        .arg("evolve")
        .arg(&f.profile)
        .arg("--against")
        .arg(&f.dataset)
        .arg("--out")
        .arg(&f.draft)
        .args(evolve_flags);
    command.output().expect("run profile evolve")
}

fn printed(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("not JSON ({e}): {}", printed(output)))
}

#[test]
fn default_output_never_contains_dataset_header_text_in_json_or_human_mode() {
    for view in ["delta", "full"] {
        for json_mode in [true, false] {
            let f = fixture(CANARY_HEADERS);
            let global: &[&str] = if json_mode { &["--json"] } else { &[] };
            let output = evolve(&f, global, &["--view", view], None);
            assert_eq!(
                output.status.code(),
                Some(0),
                "{view} json={json_mode}: {}",
                printed(&output)
            );
            let text = printed(&output);
            for canary in [NEW_COLUMN, RENAMED_KEY] {
                assert!(
                    !text.contains(canary),
                    "{view} json={json_mode}: {canary} printed: {text}"
                );
            }
            // The draft FILE is the requested artifact and keeps the real names.
            let draft = fs::read_to_string(&f.draft).unwrap();
            assert!(
                draft.contains(NEW_COLUMN) && draft.contains(RENAMED_KEY),
                "{draft}"
            );
            if json_mode {
                let result = json(&output)["result"].clone();
                assert_eq!(result["redaction"], "ordinal");
                // Header order: Canary-Loan-ID c1, canary_balance c2, NEWCOL c3. The renamed header is also
                // new to the profile, so it is listed as added as well (existing behaviour).
                assert_eq!(
                    result["added_columns"],
                    serde_json::json!(["redacted:ref:c1", "redacted:ref:c3"])
                );
                let renamed = &result["renamed_candidates"][0];
                assert_eq!(
                    renamed["old"], "canary_loan_id",
                    "the profile's own name stays"
                );
                assert_eq!(renamed["new"], "redacted:ref:c1");
                assert_eq!(
                    result["key_status"]["repaired"][0],
                    serde_json::json!({"from": "canary_loan_id", "to": "redacted:ref:c1"})
                );
                assert_eq!(
                    result["dropped_columns"],
                    serde_json::json!(["canary_loan_id"])
                );
                if view == "full" {
                    assert_eq!(
                        result["draft_profile"]["include_columns"],
                        serde_json::json!(["redacted:ref:c1", "canary_balance", "redacted:ref:c3"])
                    );
                    assert_eq!(
                        result["draft_profile"]["key"],
                        serde_json::json!(["redacted:ref:c1"])
                    );
                }
            }
        }
    }
}

#[test]
fn explicit_shows_the_names() {
    let f = fixture(CANARY_HEADERS);
    let output = evolve(&f, &["--json", "--explicit"], &[], None);
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    let result = json(&output)["result"].clone();
    assert_eq!(result["redaction"], "explicit");
    assert_eq!(
        result["added_columns"],
        serde_json::json!([RENAMED_KEY, NEW_COLUMN])
    );
    assert_eq!(result["renamed_candidates"][0]["new"], RENAMED_KEY);
    assert_eq!(
        result["key_status"]["repaired"][0],
        serde_json::json!({"from": "canary_loan_id", "to": RENAMED_KEY})
    );
    let human = evolve(&fixture(CANARY_HEADERS), &["--explicit"], &[], None);
    assert_eq!(human.status.code(), Some(0));
}

fn added(headers: &str, global: &[&str]) -> Vec<String> {
    let f = fixture(headers);
    let output = evolve(&f, global, &[], None);
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    json(&output)["result"]["added_columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn ordinals_follow_header_order_not_name_order() {
    // Two new columns, in both orders. A name-sorted numbering would make alpha_new c3 in both.
    for (headers, expected) in [
        (
            "canary_loan_id,canary_balance,zeta_new,alpha_new",
            ["zeta_new", "alpha_new"],
        ),
        (
            "canary_loan_id,canary_balance,alpha_new,zeta_new",
            ["alpha_new", "zeta_new"],
        ),
    ] {
        let named = added(headers, &["--json", "--explicit"]);
        assert_eq!(named, expected, "explicit lists header order");
        // Columns 1 and 2 are the profile's own; the new ones are c3 and c4 in file order.
        let refs = added(headers, &["--json"]);
        assert_eq!(refs, ["redacted:ref:c3", "redacted:ref:c4"], "{headers}");
    }
    // The first new column in each file is the one that came first, whatever its spelling.
    assert_eq!(
        added(
            "canary_loan_id,canary_balance,zeta_new,alpha_new",
            &["--json"]
        )[0],
        "redacted:ref:c3"
    );
}

#[test]
fn equal_header_text_shares_one_ref() {
    let f = fixture("canary_loan_id,canary_balance,dup_new,other_new,dup_new");
    let output = evolve(&f, &["--json"], &[], None);
    let result = json(&output)["result"].clone();
    // The same text twice is the same ref; the other column is its own ref.
    let refs: Vec<&str> = result["added_columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(refs.contains(&"redacted:ref:c3") && refs.contains(&"redacted:ref:c4"));
    assert!(!refs.contains(&"redacted:ref:c5"), "{refs:?}");
}

#[test]
fn default_output_is_byte_identical_across_runs() {
    let f = fixture(CANARY_HEADERS);
    let one = evolve(&f, &["--json"], &["--view", "delta"], None);
    // A second run needs a fresh draft path (evolve refuses to overwrite).
    let again = Fixture {
        _dir: temp_workspace(),
        profile: f.profile.clone(),
        dataset: f.dataset.clone(),
        draft: f.profile.with_file_name("again.draft.yaml"),
    };
    let two = evolve(&again, &["--json"], &["--view", "delta"], None);
    let strip = |o: &Output| {
        let mut v = json(o)["result"].clone();
        let object = v.as_object_mut().unwrap();
        object.remove("output_path");
        object.remove("next_actions");
        v
    };
    assert_eq!(strip(&one), strip(&two));
}

#[test]
fn keyed_markers_match_an_openssl_known_answer_and_inspect() {
    // printf 'column\0NEWCOL-Zq9x-7f2e' | openssl dgst -sha256 -hmac 0123456789abcdef | cut -c1-32
    let new_marker = "redacted:hmac-sha256:599db4f123b40c03552a06a3336c2a4d";
    // printf 'column\0Canary-Loan-ID' | openssl dgst -sha256 -hmac 0123456789abcdef | cut -c1-32
    let renamed_marker = "redacted:hmac-sha256:4afa987e0bb7f7fd09b022400524387d";
    let f = fixture(CANARY_HEADERS);
    let output = evolve(&f, &["--json"], &[], Some(KEY));
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    let result = json(&output)["result"].clone();
    assert_eq!(result["redaction"], "keyed");
    assert_eq!(
        result["added_columns"],
        serde_json::json!([renamed_marker, new_marker])
    );
    assert_eq!(result["renamed_candidates"][0]["new"], renamed_marker);
    for secret in [NEW_COLUMN, RENAMED_KEY, KEY] {
        assert!(!printed(&output).contains(secret), "{secret} printed");
    }
    // The same header gives the same marker in `profile inspect` with the same key.
    let inspect = Command::new(env!("CARGO_BIN_EXE_profile"))
        .env(KEY_ENV, KEY)
        .args(["--json", "--no-witness", "inspect"])
        .arg(&f.dataset)
        .output()
        .unwrap();
    assert_eq!(json(&inspect)["result"]["columns"][0]["id"], renamed_marker);
    // A different key changes every marker.
    let other = evolve(&fixture(CANARY_HEADERS), &["--json"], &[], Some(OTHER_KEY));
    let other = json(&other)["result"].clone();
    assert_ne!(other["added_columns"], result["added_columns"]);
    assert_ne!(
        other["renamed_candidates"][0]["new"],
        result["renamed_candidates"][0]["new"]
    );
}

#[test]
fn an_unusable_key_refuses_and_writes_no_draft() {
    for (bad, reason) in [("", "empty"), ("SHORT-KEY-xyz", "too_short")] {
        for explicit in [false, true] {
            let f = fixture(CANARY_HEADERS);
            let global: &[&str] = if explicit {
                &["--json", "--explicit"]
            } else {
                &["--json"]
            };
            let output = evolve(&f, global, &[], Some(bad));
            assert_eq!(output.status.code(), Some(2), "{bad:?} explicit={explicit}");
            let envelope = json(&output);
            assert_eq!(envelope["result"]["code"], "E_REDACTION_KEY_INVALID");
            assert_eq!(
                envelope["result"]["detail"],
                serde_json::json!({"env": KEY_ENV, "reason": reason, "min_bytes": 16})
            );
            assert!(!f.draft.exists(), "a refusal writes no draft");
            assert!(bad.is_empty() || !printed(&output).contains(bad));
        }
    }
}
