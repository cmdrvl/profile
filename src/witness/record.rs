use chrono::{SecondsFormat, Utc};
use serde_json::{Value, json};

pub fn build(
    subcommand: &str,
    inputs: Value,
    params: Value,
    output_hash: String,
    outcome: &str,
    exit_code: u8,
    prev: Option<String>,
) -> Value {
    let ts = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let id_seed = json!({
        "ts": ts,
        "subcommand": subcommand,
        "params": params,
        "output_hash": output_hash,
        "prev": prev
    });
    let seed_bytes = serde_json::to_vec(&id_seed).unwrap_or_default();
    let id = format!("blake3:{}", blake3::hash(&seed_bytes).to_hex());

    json!({
        "id": id,
        "ts": ts,
        "tool": "profile",
        "inputs": inputs,
        "params": params,
        "output_hash": output_hash,
        "outcome": outcome,
        "exit_code": exit_code,
        "prev": prev
    })
}
