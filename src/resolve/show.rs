use serde_json::{Value, json};

use crate::cli::args::ShowArgs;
use crate::refusal::RefusalPayload;
use crate::resolve::resolver::resolve_profile;

pub fn handle(profile_id: &str) -> Result<Value, RefusalPayload> {
    let resolved = resolve_profile(profile_id)?;
    Ok(json!({
        "path": resolved.path.display().to_string(),
        "profile": resolved.profile
    }))
}

pub fn run(args: &ShowArgs, _no_witness: bool) -> Result<Value, RefusalPayload> {
    handle(&args.profile_id)
}
