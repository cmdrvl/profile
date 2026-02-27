use serde_json::{Value, json};

use crate::cli::args::ListArgs;
use crate::refusal::RefusalPayload;
use crate::resolve::resolver::list_frozen_profiles;

pub fn run(_args: &ListArgs, _no_witness: bool) -> Result<Value, RefusalPayload> {
    let profiles = list_frozen_profiles()?;
    let profiles = profiles
        .into_iter()
        .map(|entry| {
            json!({
                "profile_id": entry.profile.profile_id,
                "profile_version": entry.profile.profile_version,
                "profile_family": entry.profile.profile_family,
                "profile_sha256": entry.profile.profile_sha256,
                "path": entry.path.display().to_string()
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({ "profiles": profiles }))
}
