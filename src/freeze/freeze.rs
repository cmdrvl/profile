use serde_json::{Value, json};
use std::fs;

use crate::cli::args::FreezeArgs;
use crate::refusal::RefusalPayload;
use crate::schema::{
    ProfileStatus, ValidationMode, canonical_yaml, compute_profile_sha256, is_valid_profile_family,
    parse_profile_yaml, validate_profile,
};
use crate::witness::append::append_for_command;

pub fn run(args: &FreezeArgs, no_witness: bool) -> Result<Value, RefusalPayload> {
    // Check if output file already exists
    if args.out.exists() {
        return Err(RefusalPayload::io(
            args.out.display().to_string(),
            "target file already exists - frozen profiles are immutable artifacts",
        ));
    }

    // Read and parse the draft profile
    let content = fs::read_to_string(&args.draft)
        .map_err(|error| RefusalPayload::io(args.draft.display().to_string(), error.to_string()))?;

    let mut profile = parse_profile_yaml(&content)?;

    // Check if already frozen
    if profile.is_frozen() {
        let profile_id = profile.profile_id.as_deref().unwrap_or("unknown");
        let profile_sha256 = profile.profile_sha256.as_deref().unwrap_or("unknown");
        return Err(RefusalPayload::already_frozen(profile_id, profile_sha256));
    }

    // Validate family syntax
    if !is_valid_profile_family(&args.family) {
        return Err(RefusalPayload::bad_version(
            &args.family,
            args.version,
            "invalid family syntax - must be dot-separated lowercase alphanumeric segments",
        ));
    }

    // Fill freeze defaults
    profile.fill_freeze_defaults();

    // Set identity fields
    let profile_id = format!("{}.v{}", args.family, args.version);
    profile.profile_id = Some(profile_id);
    profile.profile_version = Some(args.version);
    profile.profile_family = Some(args.family.clone());
    profile.status = ProfileStatus::Frozen;

    // Canonicalize and compute hash (needed for freeze validation)
    let canonical = canonical_yaml(&profile)?;
    let hash = compute_profile_sha256(&canonical);
    profile.profile_sha256 = Some(hash);

    // Validate the profile with freeze validation mode
    validate_profile(&profile, ValidationMode::Freeze)?;

    // Serialize the final profile (not canonical - readable format)
    let output_yaml = profile.to_yaml().map_err(|error| {
        RefusalPayload::invalid_schema_single(
            "freeze",
            format!("failed to serialize frozen profile: {error}"),
        )
    })?;

    // Write to output file
    fs::write(&args.out, output_yaml)
        .map_err(|error| RefusalPayload::io(args.out.display().to_string(), error.to_string()))?;

    let result = json!({
        "path": args.out.display().to_string(),
        "profile_id": profile.profile_id,
        "profile_sha256": profile.profile_sha256
    });
    append_for_command(
        "freeze",
        &result,
        vec![args.draft.clone()],
        json!({
            "subcommand": "freeze",
            "family": args.family,
            "version": args.version
        }),
        no_witness,
    );

    Ok(result)
}
