use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use serde_json::Value;

use crate::refusal::RefusalPayload;

pub fn append(record: &Value) -> Result<Option<String>, RefusalPayload> {
    let path = ledger_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| RefusalPayload::io(parent.display().to_string(), error.to_string()))?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| RefusalPayload::io(path.display().to_string(), error.to_string()))?;

    let mut serialized = serde_json::to_string(record)
        .map_err(|error| RefusalPayload::invalid_schema_single("witness", error.to_string()))?;
    serialized.push('\n');

    file.write_all(serialized.as_bytes())
        .map_err(|error| RefusalPayload::io(path.display().to_string(), error.to_string()))?;

    Ok(record
        .get("id")
        .and_then(Value::as_str)
        .map(std::string::ToString::to_string))
}

pub fn last_id() -> Result<Option<String>, RefusalPayload> {
    let path = ledger_path()?;
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path)
        .map_err(|error| RefusalPayload::io(path.display().to_string(), error.to_string()))?;
    let Some(last_line) = content.lines().rev().find(|line| !line.trim().is_empty()) else {
        return Ok(None);
    };

    let value: Value = serde_json::from_str(last_line)
        .map_err(|error| RefusalPayload::invalid_schema_single("witness", error.to_string()))?;
    Ok(value
        .get("id")
        .and_then(Value::as_str)
        .map(std::string::ToString::to_string))
}

pub(crate) fn ledger_path() -> Result<PathBuf, RefusalPayload> {
    ledger_path_from_env(|key| std::env::var(key).ok())
}

fn ledger_path_from_env<F>(get_env: F) -> Result<PathBuf, RefusalPayload>
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(path) = get_env("EPISTEMIC_WITNESS")
        && !path.trim().is_empty()
    {
        return Ok(PathBuf::from(path));
    }

    let home = get_env("HOME")
        .or_else(|| get_env("USERPROFILE"))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            RefusalPayload::io(
                "$HOME".to_string(),
                "HOME/USERPROFILE unavailable; set EPISTEMIC_WITNESS".to_string(),
            )
        })?;

    Ok(PathBuf::from(home).join(".epistemic").join("witness.jsonl"))
}

#[cfg(test)]
mod tests {
    use super::ledger_path_from_env;
    use std::path::PathBuf;

    #[test]
    fn ledger_path_prefers_epistemic_witness_override() {
        let path = ledger_path_from_env(|key| match key {
            "EPISTEMIC_WITNESS" => Some("/tmp/profile-witness.jsonl".to_string()),
            "HOME" => Some("/tmp/home".to_string()),
            _ => None,
        })
        .expect("override path");

        assert_eq!(path, PathBuf::from("/tmp/profile-witness.jsonl"));
    }

    #[test]
    fn ledger_path_falls_back_to_home() {
        let path = ledger_path_from_env(|key| match key {
            "EPISTEMIC_WITNESS" => Some(String::new()),
            "HOME" => Some("/tmp/home".to_string()),
            _ => None,
        })
        .expect("home path");

        assert_eq!(path, PathBuf::from("/tmp/home/.epistemic/witness.jsonl"));
    }

    #[test]
    fn ledger_path_errors_without_override_or_home() {
        let error = ledger_path_from_env(|_| None).expect_err("missing path should refuse");
        assert_eq!(error.code, "E_IO");
    }
}
