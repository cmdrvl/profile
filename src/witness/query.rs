use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::cli::args::{WitnessCountArgs, WitnessLastArgs, WitnessQueryArgs};
use crate::refusal::RefusalPayload;

pub fn run_query(args: &WitnessQueryArgs) -> Result<Value, RefusalPayload> {
    let mut records = read_ledger_records()?;
    records.reverse();

    if let Some(limit) = args.limit {
        records.truncate(limit);
    }

    Ok(json!({ "records": records }))
}

pub fn run_last(args: &WitnessLastArgs) -> Result<Value, RefusalPayload> {
    let mut records = read_ledger_records()?;
    records.reverse();
    records.truncate(args.count);

    Ok(json!({ "records": records }))
}

pub fn run_count(_args: &WitnessCountArgs) -> Result<Value, RefusalPayload> {
    let records = read_ledger_records()?;
    Ok(json!({ "count": records.len() }))
}

fn read_ledger_records() -> Result<Vec<Value>, RefusalPayload> {
    let path = ledger_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&path)
        .map_err(|error| RefusalPayload::io(path.display().to_string(), error.to_string()))?;

    let mut records = Vec::new();
    for (index, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        let value = serde_json::from_str::<Value>(line).map_err(|error| {
            RefusalPayload::invalid_schema_single(
                "witness",
                format!("invalid ledger JSON at line {}: {}", index + 1, error),
            )
        })?;
        records.push(value);
    }

    Ok(records)
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
