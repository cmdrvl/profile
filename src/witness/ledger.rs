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

fn ledger_path() -> Result<PathBuf, RefusalPayload> {
    let home = std::env::var("HOME").map_err(|error| {
        RefusalPayload::io(
            "$HOME".to_string(),
            format!("HOME environment variable unavailable: {error}"),
        )
    })?;
    Ok(PathBuf::from(home).join(".epistemic").join("witness.jsonl"))
}
