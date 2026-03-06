use std::fs;

use serde_json::json;

use crate::cli::args::ValidateArgs;
use crate::output::json::{CommandOutput, ProfileRef};
use crate::refusal::RefusalPayload;
use crate::schema::{ValidationMode, parse_profile_yaml, validate_profile};
use crate::witness::append::append_for_command;

pub fn run(args: &ValidateArgs, no_witness: bool) -> Result<CommandOutput, RefusalPayload> {
    let path = args.file.display().to_string();
    let content = fs::read_to_string(&args.file)
        .map_err(|error| RefusalPayload::io(path, error.to_string()))?;
    let profile = parse_profile_yaml(&content)?;
    validate_profile(&profile, ValidationMode::Validate)?;

    let result = json!({
        "valid": true
    });
    let witness_id = append_for_command(
        "validate",
        &result,
        vec![args.file.clone()],
        json!({ "subcommand": "validate" }),
        no_witness,
    );

    Ok(CommandOutput::success(result)
        .with_profile_ref(ProfileRef::from_profile(&profile))
        .with_witness_id(witness_id))
}
