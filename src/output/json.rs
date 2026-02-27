use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cli::exit::{EXIT_ISSUES_FOUND, EXIT_REFUSAL, EXIT_SUCCESS};
use crate::refusal::RefusalPayload;

/// Unified output envelope for all --json responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputEnvelope {
    /// API version for this envelope format
    pub version: u32,
    /// Operation outcome: SUCCESS, ISSUES_FOUND, or REFUSAL
    pub outcome: String,
    /// Exit code (0, 1, or 2)
    pub exit_code: u8,
    /// The subcommand that produced this output
    pub subcommand: String,
    /// The actual result payload (subcommand-specific)
    pub result: Value,
    /// Reference to the profile that was consumed, if any
    pub profile_ref: Option<String>,
    /// ID of witness record if written, null if --no-witness or non-witnessed command
    pub witness_id: Option<String>,
}

impl OutputEnvelope {
    /// Create a new envelope for a successful operation
    pub fn success(subcommand: String, result: Value) -> Self {
        Self {
            version: 1,
            outcome: "SUCCESS".to_string(),
            exit_code: EXIT_SUCCESS,
            subcommand,
            result,
            profile_ref: None,
            witness_id: None,
        }
    }

    /// Create a new envelope for an operation that found issues
    pub fn issues_found(subcommand: String, result: Value) -> Self {
        Self {
            version: 1,
            outcome: "ISSUES_FOUND".to_string(),
            exit_code: EXIT_ISSUES_FOUND,
            subcommand,
            result,
            profile_ref: None,
            witness_id: None,
        }
    }

    /// Create a new envelope for a refused operation
    pub fn refusal(subcommand: String, refusal: RefusalPayload) -> Self {
        Self {
            version: 1,
            outcome: "REFUSAL".to_string(),
            exit_code: EXIT_REFUSAL,
            subcommand,
            result: serde_json::to_value(refusal).unwrap_or(Value::Null),
            profile_ref: None,
            witness_id: None,
        }
    }

    /// Set the profile reference
    pub fn with_profile_ref(mut self, profile_ref: Option<String>) -> Self {
        self.profile_ref = profile_ref;
        self
    }

    /// Set the witness ID
    pub fn with_witness_id(mut self, witness_id: Option<String>) -> Self {
        self.witness_id = witness_id;
        self
    }
}

/// Emit JSON output with unified envelope format
pub fn emit(subcommand: &str, result: Result<Value, RefusalPayload>) -> u8 {
    let envelope = match result {
        Ok(value) => {
            if is_issues_found(subcommand, &value) {
                OutputEnvelope::issues_found(subcommand.to_string(), value)
            } else {
                OutputEnvelope::success(subcommand.to_string(), value)
            }
        }
        Err(refusal) => OutputEnvelope::refusal(subcommand.to_string(), refusal),
    };

    match serde_json::to_string_pretty(&envelope) {
        Ok(json) => {
            println!("{}", json);
            envelope.exit_code
        }
        Err(_) => {
            eprintln!("Failed to serialize output envelope");
            EXIT_REFUSAL
        }
    }
}

fn is_issues_found(subcommand: &str, value: &Value) -> bool {
    match subcommand {
        "lint" => value
            .get("issues")
            .and_then(Value::as_array)
            .is_some_and(|issues| !issues.is_empty()),
        "diff" => value
            .get("differences")
            .or_else(|| value.get("changes"))
            .and_then(Value::as_array)
            .is_some_and(|changes| !changes.is_empty()),
        _ => false,
    }
}
