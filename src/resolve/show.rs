use serde_json::json;

use crate::cli::args::ShowArgs;
use crate::output::json::{CommandOutput, ProfileRef};
use crate::refusal::RefusalPayload;
use crate::resolve::resolver::resolve_profile;

pub fn handle(profile_id: &str) -> Result<CommandOutput, RefusalPayload> {
    let resolved = resolve_profile(profile_id)?;
    let result = json!({
        "path": resolved.path.display().to_string(),
        "profile": resolved.profile
    });

    Ok(
        CommandOutput::success(result)
            .with_profile_ref(ProfileRef::from_profile(&resolved.profile)),
    )
}

pub fn run(args: &ShowArgs, _no_witness: bool) -> Result<CommandOutput, RefusalPayload> {
    handle(&args.profile_id)
}
