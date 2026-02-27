use std::path::Path;
use std::{fs, path::PathBuf};

use serde_json::{Value, json};

use crate::cli::args::PullArgs;
use crate::network::get_text;
use crate::refusal::payload::RefusalPayload;
use crate::schema::{ValidationMode, parse_profile_yaml, validate_profile};

pub fn run(args: &PullArgs, _no_witness: bool) -> Result<Value, RefusalPayload> {
    handle_pull(&args.profile_id, &args.out)
}

pub fn handle_pull(profile_id: &str, out: &Path) -> Result<Value, RefusalPayload> {
    let response = get_text(&format!("/query/profile/{profile_id}"))?;
    let content = extract_profile_content(&response)?;

    let profile = parse_profile_yaml(&content)?;
    validate_profile(&profile, ValidationMode::Validate)?;
    if !profile.is_frozen() {
        return Err(RefusalPayload::invalid_schema_single(
            "status",
            "pull only accepts frozen profiles",
        ));
    }

    let resolved_id = profile
        .profile_id
        .clone()
        .ok_or_else(|| RefusalPayload::missing_field("profile_id"))?;
    if resolved_id != profile_id {
        return Err(RefusalPayload::invalid_schema_single(
            "profile_id",
            format!("pulled profile_id '{resolved_id}' did not match requested '{profile_id}'"),
        ));
    }

    fs::create_dir_all(out)
        .map_err(|error| RefusalPayload::io(out.display().to_string(), error.to_string()))?;

    let output_path = output_path(out, profile_id);
    if output_path.exists() {
        return Err(RefusalPayload::io(
            output_path.display().to_string(),
            "target file already exists".to_string(),
        ));
    }

    fs::write(&output_path, content).map_err(|error| {
        RefusalPayload::io(output_path.display().to_string(), error.to_string())
    })?;

    Ok(json!({
        "fetched": true,
        "profile_id": profile_id,
        "profile_sha256": profile.profile_sha256,
        "path": output_path.display().to_string()
    }))
}

fn output_path(out_dir: &Path, profile_id: &str) -> PathBuf {
    out_dir.join(format!("{profile_id}.yaml"))
}

fn extract_profile_content(response: &str) -> Result<String, RefusalPayload> {
    let trimmed = response.trim();
    if trimmed.is_empty() {
        return Err(RefusalPayload::io(
            "fabric:/query/profile".to_string(),
            "empty response body".to_string(),
        ));
    }

    if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
        return Ok(trimmed.to_string());
    }

    let parsed = serde_json::from_str::<Value>(trimmed).map_err(|error| {
        RefusalPayload::io(
            "fabric:/query/profile".to_string(),
            format!("invalid JSON response: {error}"),
        )
    })?;

    if let Some(content) = parsed.get("content").and_then(Value::as_str) {
        return Ok(content.to_string());
    }
    if let Some(content) = parsed.get("profile_yaml").and_then(Value::as_str) {
        return Ok(content.to_string());
    }
    if let Some(content) = parsed
        .get("result")
        .and_then(|result| result.get("content"))
        .and_then(Value::as_str)
    {
        return Ok(content.to_string());
    }

    if let Some(profile) = parsed.get("profile") {
        return serde_yaml::to_string(profile).map_err(|error| {
            RefusalPayload::io(
                "fabric:/query/profile".to_string(),
                format!("failed to encode profile YAML: {error}"),
            )
        });
    }

    if let Some(profile) = parsed
        .get("result")
        .and_then(|result| result.get("profile"))
    {
        return serde_yaml::to_string(profile).map_err(|error| {
            RefusalPayload::io(
                "fabric:/query/profile".to_string(),
                format!("failed to encode profile YAML: {error}"),
            )
        });
    }

    Err(RefusalPayload::io(
        "fabric:/query/profile".to_string(),
        "response missing profile content".to_string(),
    ))
}
