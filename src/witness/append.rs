use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::witness::{ledger, record};

pub fn append_for_command(
    subcommand: &str,
    result: &Value,
    input_paths: Vec<PathBuf>,
    params: Value,
    no_witness: bool,
) {
    if no_witness || !witness_enabled_subcommand(subcommand) {
        return;
    }

    let outcome = if subcommand == "lint"
        && result
            .get("issues")
            .and_then(Value::as_array)
            .is_some_and(|issues| !issues.is_empty())
    {
        "ISSUES_FOUND"
    } else {
        "SUCCESS"
    };
    let exit_code = if outcome == "ISSUES_FOUND" { 1 } else { 0 };

    let inputs = match build_inputs(&input_paths) {
        Ok(inputs) => inputs,
        Err(error) => {
            eprintln!("Warning: witness append skipped (input metadata failed): {error}");
            return;
        }
    };

    let output_hash = match build_output_hash(subcommand, result) {
        Ok(hash) => hash,
        Err(error) => {
            eprintln!("Warning: witness append skipped (output hash failed): {error}");
            return;
        }
    };

    let prev = match ledger::last_id() {
        Ok(prev) => prev,
        Err(error) => {
            eprintln!("Warning: witness append skipped (read previous id failed): {error}");
            return;
        }
    };

    let record = record::build(
        subcommand,
        inputs,
        params,
        output_hash,
        outcome,
        exit_code,
        prev,
    );

    if let Err(error) = ledger::append(&record) {
        eprintln!("Warning: witness append failed: {error}");
    }
}

fn witness_enabled_subcommand(subcommand: &str) -> bool {
    matches!(
        subcommand,
        "freeze" | "validate" | "lint" | "stats" | "suggest-key"
    )
}

fn build_inputs(input_paths: &[PathBuf]) -> Result<Value, String> {
    let mut inputs = Vec::with_capacity(input_paths.len());
    for path in input_paths {
        let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        inputs.push(json!({
            "path": path.display().to_string(),
            "hash": format!("blake3:{}", blake3::hash(&bytes).to_hex()),
            "bytes": bytes.len()
        }));
    }
    Ok(Value::Array(inputs))
}

fn build_output_hash(subcommand: &str, result: &Value) -> Result<String, String> {
    let bytes = if subcommand == "freeze" {
        let path = result
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| "freeze result missing path".to_string())?;
        fs::read(path).map_err(|error| format!("{path}: {error}"))?
    } else {
        serde_json::to_vec(result).map_err(|error| error.to_string())?
    };

    Ok(format!("blake3:{}", blake3::hash(&bytes).to_hex()))
}
