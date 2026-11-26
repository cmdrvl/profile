mod common;

use std::fs;

use common::{
    assert_json_envelope_shape, copy_fixture, parse_stdout_json, profile_cmd, temp_workspace,
};
use serde_yaml::Value as YamlValue;

#[test]
fn evolve_writes_a_valid_next_draft_and_preserves_frozen_input() {
    let workspace = temp_workspace();
    let old_path = workspace.path().join("old.yaml");
    let dataset_path = workspace.path().join("new.csv");
    let draft_path = workspace.path().join("next.draft.yaml");
    write_frozen_profile(
        &old_path,
        "csv.loan_tape.core",
        0,
        &["loan_id", "balance"],
        &["loan_id"],
    );
    fs::write(
        &dataset_path,
        "loan_id,balance,rate\nLN-1,10,0.05\nLN-2,20,0.06\n",
    )
    .expect("new dataset should be written");
    let original = fs::read(&old_path).expect("frozen input should be readable");

    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("--explicit")
        .arg("evolve")
        .arg(&old_path)
        .arg("--against")
        .arg(&dataset_path)
        .arg("--out")
        .arg(&draft_path)
        .arg("--view")
        .arg("delta")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);
    assert_json_envelope_shape(&envelope);

    assert_eq!(envelope["subcommand"], "evolve");
    assert_eq!(envelope["result"]["next_profile_version"], 1);
    assert_eq!(
        envelope["result"]["added_columns"],
        serde_json::json!(["rate"])
    );
    assert!(
        envelope["result"]["dropped_columns"]
            .as_array()
            .expect("dropped columns should be an array")
            .is_empty()
    );
    assert_eq!(
        fs::read(&old_path).expect("frozen input should remain readable"),
        original
    );

    let draft: YamlValue =
        serde_yaml::from_str(&fs::read_to_string(&draft_path).expect("draft should be written"))
            .expect("draft YAML should parse");
    assert_eq!(draft["status"].as_str(), Some("draft"));
    assert_eq!(draft["profile_version"], YamlValue::Null);
    assert_eq!(
        draft["include_columns"],
        serde_yaml::from_str::<YamlValue>("[loan_id, balance, rate]").unwrap()
    );

    let validate = profile_cmd()
        .arg("--no-witness")
        .arg("validate")
        .arg(&draft_path)
        .assert();
    common::assert_success_exit!(validate);
}

#[test]
fn evolve_reports_registry_alias_as_a_rename_without_dropping_the_column() {
    let workspace = temp_workspace();
    let registry_dir = workspace.path().join("registries").join("annex_columns_v0");
    copy_fixture(
        "registries/annex_columns_v0/registry.json",
        registry_dir.join("registry.json"),
    );
    copy_fixture(
        "registries/annex_columns_v0/aliases.json",
        registry_dir.join("aliases.json"),
    );

    let draft_path = workspace.path().join("source.draft.yaml");
    fs::write(
        &draft_path,
        "schema_version: 1\nstatus: draft\nformat: csv\ncolumn_registry: registries/annex_columns_v0\nkey:\n  - loan_id_number\ninclude_columns:\n  - loan_id_number\n  - current_balance\n",
    )
    .expect("source draft should be written");
    let old_path = workspace.path().join("old.yaml");
    let freeze = profile_cmd()
        .arg("--no-witness")
        .arg("freeze")
        .arg(&draft_path)
        .arg("--family")
        .arg("csv.loan_tape.registry")
        .arg("--version")
        .arg("0")
        .arg("--out")
        .arg(&old_path)
        .assert();
    common::assert_success_exit!(freeze);

    let dataset_path = workspace.path().join("renamed.csv");
    fs::write(
        &dataset_path,
        "Loan ID Number,Current Balance\nA,100\nB,200\n",
    )
    .expect("renamed dataset should be written");
    let out_path = workspace.path().join("next.yaml");
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("--explicit")
        .arg("evolve")
        .arg(&old_path)
        .arg("--against")
        .arg(&dataset_path)
        .arg("--out")
        .arg(&out_path)
        .arg("--view")
        .arg("delta")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    assert!(
        envelope["result"]["added_columns"]
            .as_array()
            .expect("added columns should be an array")
            .is_empty()
    );
    assert!(
        envelope["result"]["dropped_columns"]
            .as_array()
            .expect("dropped columns should be an array")
            .is_empty()
    );
    let candidates = envelope["result"]["renamed_candidates"]
        .as_array()
        .expect("rename candidates should be an array");
    assert!(candidates.iter().any(|candidate| {
        candidate["source"] == "column_registry"
            && candidate["old"] == "loan_id_number"
            && candidate["new"] == "Loan ID Number"
            && candidate["ambiguous"] == false
    }));
}

#[test]
fn evolve_flags_ambiguous_similarity_and_does_not_guess_the_key() {
    let workspace = temp_workspace();
    let old_path = workspace.path().join("old.yaml");
    let dataset_path = workspace.path().join("ambiguous.csv");
    let out_path = workspace.path().join("next.yaml");
    write_frozen_profile(&old_path, "csv.members", 3, &["member"], &["member"]);
    fs::write(&dataset_path, "member_id,member_no\nA,1\nB,2\n")
        .expect("ambiguous dataset should be written");

    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("evolve")
        .arg(&old_path)
        .arg("--against")
        .arg(&dataset_path)
        .arg("--out")
        .arg(&out_path)
        .arg("--view")
        .arg("delta")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);

    let candidate = envelope["result"]["renamed_candidates"]
        .as_array()
        .and_then(|candidates| candidates.first())
        .expect("an ambiguous rename should be reported");
    assert_eq!(candidate["old"], "member");
    assert_eq!(candidate["ambiguous"], true);
    let draft: YamlValue =
        serde_yaml::from_str(&fs::read_to_string(&out_path).expect("draft should be written"))
            .expect("draft YAML should parse");
    assert_eq!(
        draft["key"],
        serde_yaml::from_str::<YamlValue>("[member]").unwrap()
    );
}

#[test]
fn evolve_applies_peek_slice_shift_and_followed_by_freeze() {
    let workspace = temp_workspace();
    let old_draft = workspace.path().join("source.draft.yaml");
    fs::write(
        &old_draft,
        "schema_version: 1\nstatus: draft\nformat: csv\npre_parse:\n  expected_shape:\n    modal_column_count: 2\n    first_data_row: 4\n  slice:\n    mode: preamble_skip\n    skip_rows: 2\n    header_at_row: 3\n    data_starts_at: 4\n    preamble_capture: true\ninclude_columns:\n  - Account ID\n  - Amount\n",
    )
    .expect("old draft should be written");
    let old_path = workspace.path().join("old.yaml");
    let freeze_old = profile_cmd()
        .arg("--no-witness")
        .arg("freeze")
        .arg(&old_draft)
        .arg("--family")
        .arg("csv.accounts")
        .arg("--version")
        .arg("0")
        .arg("--out")
        .arg(&old_path)
        .assert();
    common::assert_success_exit!(freeze_old);

    let dataset_path = workspace.path().join("shifted.csv");
    fs::write(
        &dataset_path,
        "New vendor note\nAnother note\nThird note\nAccount ID,Amount\nA,10\nB,20\n",
    )
    .expect("shifted dataset should be written");
    let peek_path = workspace.path().join("peek.json");
    fs::write(
        &peek_path,
        r#"{
  "result": {
    "summary": {"modal_column_count": 2, "data_starts_at": 5},
    "suggestions": {"profile_pre_parse": {
      "mode": "preamble_skip", "skip_rows": 3,
      "header_at_row": 4, "data_starts_at": 5
    }}
  }
}"#,
    )
    .expect("peek JSON should be written");
    let next_draft = workspace.path().join("next.draft.yaml");
    let assert = profile_cmd()
        .arg("--json")
        .arg("--no-witness")
        .arg("evolve")
        .arg(&old_path)
        .arg("--against")
        .arg(&dataset_path)
        .arg("--out")
        .arg(&next_draft)
        .arg("--from-peek")
        .arg(&peek_path)
        .arg("--view")
        .arg("delta")
        .assert();
    let envelope = parse_stdout_json(&assert);
    common::assert_success_exit!(assert);
    assert_eq!(envelope["result"]["slice_shift"]["changed"], true);
    assert_eq!(envelope["result"]["slice_shift"]["new"]["header_at_row"], 4);

    let frozen_next = workspace.path().join("next.yaml");
    let freeze_next = profile_cmd()
        .arg("--no-witness")
        .arg("freeze")
        .arg(&next_draft)
        .arg("--family")
        .arg("csv.accounts")
        .arg("--version")
        .arg("1")
        .arg("--out")
        .arg(&frozen_next)
        .assert();
    common::assert_success_exit!(freeze_next);
}

fn write_frozen_profile(
    path: &std::path::Path,
    family: &str,
    version: u64,
    include_columns: &[&str],
    key: &[&str],
) {
    let include = include_columns
        .iter()
        .map(|column| format!("  - {column}"))
        .collect::<Vec<_>>()
        .join("\n");
    let key = key
        .iter()
        .map(|column| format!("  - {column}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        path,
        format!(
            "schema_version: 1\nprofile_id: {family}.v{version}\nprofile_version: {version}\nprofile_family: {family}\nprofile_sha256: sha256:{:0<64}\nstatus: frozen\nformat: csv\nkey:\n{key}\ninclude_columns:\n{include}\n",
            "1"
        ),
    )
    .expect("frozen profile should be written");
}
