use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::refusal::RefusalPayload;
use crate::schema::{Profile, ProfileStatus, ValidationMode, parse_profile_yaml, validate_profile};

#[derive(Debug, Clone)]
pub struct ResolvedProfile {
    pub path: PathBuf,
    pub profile: Profile,
}

pub fn resolve(profile_ref: &str) -> Result<Value, RefusalPayload> {
    let resolved = resolve_profile(profile_ref)?;
    Ok(json!({
        "path": resolved.path.display().to_string(),
        "profile": resolved.profile
    }))
}

pub fn resolve_profile(profile_ref: &str) -> Result<ResolvedProfile, RefusalPayload> {
    let input_path = Path::new(profile_ref);
    if input_path.exists() {
        return parse_profile_from_path(input_path);
    }

    let profiles = list_frozen_profiles()?;
    profiles
        .into_iter()
        .find(|entry| entry.profile.profile_id.as_deref() == Some(profile_ref))
        .ok_or_else(|| {
            RefusalPayload::io(
                profile_ref.to_string(),
                "profile not found in ~/.epistemic/profiles".to_string(),
            )
        })
}

pub fn list_frozen_profiles() -> Result<Vec<ResolvedProfile>, RefusalPayload> {
    let mut entries = Vec::new();
    let directory = default_profile_directory()?;

    if !directory.exists() {
        return Ok(entries);
    }

    let read_dir = fs::read_dir(&directory)
        .map_err(|error| RefusalPayload::io(directory.display().to_string(), error.to_string()))?;

    for entry in read_dir {
        let entry = entry.map_err(|error| {
            RefusalPayload::io(directory.display().to_string(), error.to_string())
        })?;
        let path = entry.path();

        if path.extension().and_then(|ext| ext.to_str()) != Some("yaml") {
            continue;
        }

        if let Ok(resolved) = parse_profile_from_path(&path)
            && matches!(resolved.profile.status, ProfileStatus::Frozen)
        {
            entries.push(resolved);
        }
    }

    entries.sort_by(|left, right| {
        left.profile
            .profile_family
            .as_deref()
            .unwrap_or("")
            .cmp(right.profile.profile_family.as_deref().unwrap_or(""))
            .then(
                left.profile
                    .profile_version
                    .cmp(&right.profile.profile_version),
            )
            .then(left.path.cmp(&right.path))
    });

    Ok(entries)
}

fn parse_profile_from_path(path: &Path) -> Result<ResolvedProfile, RefusalPayload> {
    let content = fs::read_to_string(path)
        .map_err(|error| RefusalPayload::io(path.display().to_string(), error.to_string()))?;
    let profile = parse_profile_yaml(&content)?;
    validate_profile(&profile, ValidationMode::Validate)?;

    Ok(ResolvedProfile {
        path: path.to_path_buf(),
        profile,
    })
}

fn default_profile_directory() -> Result<PathBuf, RefusalPayload> {
    let home = std::env::var("HOME").map_err(|error| {
        RefusalPayload::io(
            "$HOME".to_string(),
            format!("HOME environment variable unavailable: {error}"),
        )
    })?;
    Ok(PathBuf::from(home).join(".epistemic").join("profiles"))
}
