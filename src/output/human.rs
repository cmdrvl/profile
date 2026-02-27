use serde_json::Value;

use crate::cli::exit::{EXIT_REFUSAL, EXIT_SUCCESS};
use crate::refusal::RefusalPayload;

pub fn emit(_subcommand: &str, result: Result<Value, RefusalPayload>) -> u8 {
    match result {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
            );
            EXIT_SUCCESS
        }
        Err(refusal) => {
            eprintln!("Error: {}", refusal);
            EXIT_REFUSAL
        }
    }
}
