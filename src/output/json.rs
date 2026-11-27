use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cli::exit::{EXIT_ISSUES_FOUND, EXIT_REFUSAL, EXIT_SUCCESS};
use crate::refusal::RefusalPayload;
use crate::schema::Profile;

const ENVELOPE_VERSION: &str = "profile.v0";

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub result: Value,
    /// Optional raw JSON result for a command whose public JSON contract is
    /// intentionally outside the normal profile.v0 envelope.
    pub bare_json_result: Option<Value>,
    pub profile_ref: Option<ProfileRef>,
    pub witness_id: Option<String>,
}

impl CommandOutput {
    pub fn success(result: Value) -> Self {
        Self {
            result,
            bare_json_result: None,
            profile_ref: None,
            witness_id: None,
        }
    }

    pub fn with_bare_json_result(mut self, result: Value) -> Self {
        self.bare_json_result = Some(result);
        self
    }

    pub fn with_profile_ref(mut self, profile_ref: Option<ProfileRef>) -> Self {
        self.profile_ref = profile_ref;
        self
    }

    pub fn with_witness_id(mut self, witness_id: Option<String>) -> Self {
        self.witness_id = witness_id;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileRef {
    pub profile_id: String,
    pub profile_sha256: String,
}

impl ProfileRef {
    pub fn from_profile(profile: &Profile) -> Option<Self> {
        Some(Self {
            profile_id: profile.profile_id.clone()?,
            profile_sha256: profile.profile_sha256.clone()?,
        })
    }
}

/// Unified output envelope for all --json responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputEnvelope {
    /// API version for this envelope format
    pub version: String,
    /// Operation outcome: SUCCESS, ISSUES_FOUND, or REFUSAL
    pub outcome: String,
    /// Exit code (0, 1, or 2)
    pub exit_code: u8,
    /// The subcommand that produced this output
    pub subcommand: String,
    /// The actual result payload (subcommand-specific)
    pub result: Value,
    /// Reference to the profile that was consumed, if any
    pub profile_ref: Option<ProfileRef>,
    /// ID of witness record if written, null if --no-witness or non-witnessed command
    pub witness_id: Option<String>,
}

impl OutputEnvelope {
    /// Create a new envelope for a successful operation
    pub fn success(subcommand: String, output: CommandOutput) -> Self {
        Self {
            version: ENVELOPE_VERSION.to_string(),
            outcome: "SUCCESS".to_string(),
            exit_code: EXIT_SUCCESS,
            subcommand,
            result: output.result,
            profile_ref: output.profile_ref,
            witness_id: output.witness_id,
        }
    }

    /// Create a new envelope for an operation that found issues
    pub fn issues_found(subcommand: String, output: CommandOutput) -> Self {
        Self {
            version: ENVELOPE_VERSION.to_string(),
            outcome: "ISSUES_FOUND".to_string(),
            exit_code: EXIT_ISSUES_FOUND,
            subcommand,
            result: output.result,
            profile_ref: output.profile_ref,
            witness_id: output.witness_id,
        }
    }

    /// Create a new envelope for a refused operation
    pub fn refusal(subcommand: String, refusal: RefusalPayload) -> Self {
        Self {
            version: ENVELOPE_VERSION.to_string(),
            outcome: "REFUSAL".to_string(),
            exit_code: EXIT_REFUSAL,
            subcommand,
            result: serde_json::to_value(refusal).unwrap_or(Value::Null),
            profile_ref: None,
            witness_id: None,
        }
    }
}

/// Emit JSON output with unified envelope format
pub fn emit(subcommand: &str, result: Result<CommandOutput, RefusalPayload>) -> u8 {
    if let Ok(output) = &result
        && subcommand.starts_with("witness ")
        && let Some(bare_result) = &output.bare_json_result
    {
        match serde_json::to_string(bare_result) {
            Ok(json) => {
                println!("{json}");
                return EXIT_SUCCESS;
            }
            Err(_) => {
                eprintln!("Failed to serialize witness output");
                return EXIT_REFUSAL;
            }
        }
    }

    let envelope = match result {
        Ok(output) => {
            if is_issues_found(subcommand, &output.result) {
                OutputEnvelope::issues_found(subcommand.to_string(), output)
            } else {
                OutputEnvelope::success(subcommand.to_string(), output)
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
