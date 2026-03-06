use chrono::{SecondsFormat, Utc};
use serde_json::{Value, json};

pub fn build(
    _subcommand: &str,
    inputs: Value,
    params: Value,
    output_hash: String,
    outcome: &str,
    exit_code: u8,
    prev: Option<String>,
) -> Value {
    let ts = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let binary_hash = hash_self()
        .map(|value| format!("blake3:{value}"))
        .unwrap_or_default();

    let mut record = json!({
        "id": "",
        "ts": ts,
        "tool": "profile",
        "version": env!("CARGO_PKG_VERSION"),
        "binary_hash": binary_hash,
        "inputs": inputs,
        "params": params,
        "output_hash": output_hash,
        "outcome": outcome,
        "exit_code": exit_code,
        "prev": prev
    });

    let seed_bytes = serde_json::to_vec(&record).unwrap_or_default();
    record["id"] = Value::String(format!("blake3:{}", blake3::hash(&seed_bytes).to_hex()));
    record
}

fn hash_self() -> Result<String, std::io::Error> {
    let path = std::env::current_exe()?;
    let bytes = std::fs::read(path)?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}
