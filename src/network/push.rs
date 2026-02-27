use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::cli::args::PushArgs;
use crate::network::post_json;
use crate::refusal::payload::RefusalPayload;
use crate::schema::{ValidationMode, parse_profile_yaml, validate_profile};

pub fn run(args: &PushArgs, _no_witness: bool) -> Result<Value, RefusalPayload> {
    handle_push(&args.file)
}

pub fn handle_push(file: &Path) -> Result<Value, RefusalPayload> {
    let content = fs::read_to_string(file)
        .map_err(|error| RefusalPayload::io(file.display().to_string(), error.to_string()))?;

    let profile = parse_profile_yaml(&content)?;
    validate_profile(&profile, ValidationMode::Validate)?;
    if !profile.is_frozen() {
        return Err(RefusalPayload::invalid_schema_single(
            "status",
            "push requires a frozen profile",
        ));
    }

    let profile_id = profile
        .profile_id
        .clone()
        .ok_or_else(|| RefusalPayload::missing_field("profile_id"))?;
    let profile_sha256 = profile
        .profile_sha256
        .clone()
        .ok_or_else(|| RefusalPayload::missing_field("profile_sha256"))?;
    let profile_family = profile
        .profile_family
        .clone()
        .ok_or_else(|| RefusalPayload::missing_field("profile_family"))?;
    let profile_version = profile
        .profile_version
        .ok_or_else(|| RefusalPayload::missing_field("profile_version"))?;

    let command = json!({
        "command": "AddProfileArtifact",
        "payload": {
            "profile_id": profile_id,
            "profile_sha256": profile_sha256,
            "profile_family": profile_family,
            "profile_version": profile_version,
            "format": profile.format.as_str(),
            "schema_version": profile.schema_version,
            "content": content
        }
    });

    let response_body = post_json("/execute", &command)?;
    assert_no_fabric_errors(&response_body)?;

    Ok(json!({
        "published": true,
        "profile_id": profile_id,
        "profile_sha256": profile_sha256,
        "source_path": file.display().to_string()
    }))
}

fn assert_no_fabric_errors(body: &str) -> Result<(), RefusalPayload> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
        return Ok(());
    };

    let Some(errors) = value.get("errors") else {
        return Ok(());
    };

    if errors.is_null() {
        return Ok(());
    }

    if let Some(list) = errors.as_array()
        && list.is_empty()
    {
        return Ok(());
    }

    Err(RefusalPayload::io(
        "fabric:/execute".to_string(),
        format!("data-fabric reported errors: {errors}"),
    ))
}
