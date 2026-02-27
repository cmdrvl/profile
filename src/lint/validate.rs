use std::fs;

use serde_json::{Value, json};

use crate::cli::args::ValidateArgs;
use crate::refusal::RefusalPayload;
use crate::schema::{ValidationMode, parse_profile_yaml, validate_profile};
use crate::witness::append::append_for_command;

pub fn run(args: &ValidateArgs, no_witness: bool) -> Result<Value, RefusalPayload> {
    let path = args.file.display().to_string();
    let content = fs::read_to_string(&args.file)
        .map_err(|error| RefusalPayload::io(path, error.to_string()))?;
    let profile = parse_profile_yaml(&content)?;
    validate_profile(&profile, ValidationMode::Validate)?;

    let result = json!({
        "valid": true
    });
    append_for_command(
        "validate",
        &result,
        vec![args.file.clone()],
        json!({ "subcommand": "validate" }),
        no_witness,
    );

    Ok(result)
}
