use crate::refusal::RefusalPayload;
use crate::schema::profile::{HashAlgorithm, Profile, ProfileStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    Validate,
    Freeze,
}

pub fn parse_profile_yaml(content: &str) -> Result<Profile, RefusalPayload> {
    Profile::from_yaml(content).map_err(map_yaml_error)
}

pub fn validate_profile(profile: &Profile, mode: ValidationMode) -> Result<(), RefusalPayload> {
    if profile.schema_version != 1 {
        return Err(invalid_schema("schema_version", "must be 1"));
    }

    if !matches!(profile.format, crate::schema::ProfileFormat::Csv) {
        return Err(invalid_schema("format", "must be csv"));
    }

    if profile.key.iter().any(|column| column.trim().is_empty()) {
        return Err(invalid_schema("key", "columns must be non-empty strings"));
    }

    if profile
        .include_columns
        .iter()
        .any(|column| column.trim().is_empty())
    {
        return Err(invalid_schema(
            "include_columns",
            "entries must be non-empty strings",
        ));
    }

    if matches!(mode, ValidationMode::Freeze) && profile.include_columns.is_empty() {
        return Err(invalid_schema(
            "include_columns",
            "must be non-empty for freeze",
        ));
    }

    if profile.is_frozen() {
        let family = profile
            .profile_family
            .as_deref()
            .ok_or_else(|| missing_field("profile_family"))?;
        let version = profile
            .profile_version
            .ok_or_else(|| missing_field("profile_version"))?;
        let profile_id = profile
            .profile_id
            .as_deref()
            .ok_or_else(|| missing_field("profile_id"))?;

        if !is_valid_profile_family(family) {
            return Err(invalid_schema(
                "profile_family",
                "profile_family is not in valid family format",
            ));
        }

        let expected_id = format!("{family}.v{version}");
        if profile_id != expected_id {
            return Err(invalid_schema(
                "profile_id",
                "profile_id must equal <profile_family>.v<profile_version>",
            ));
        }

        let profile_sha256 = profile.profile_sha256.as_deref();
        if matches!(mode, ValidationMode::Validate) {
            let sha = profile_sha256.ok_or_else(|| missing_field("profile_sha256"))?;

            if !is_valid_profile_sha256(sha) {
                return Err(invalid_schema(
                    "profile_sha256",
                    "profile_sha256 must match sha256:<64 lowercase hex chars>",
                ));
            }
        } else if let Some(sha) = profile_sha256
            && !is_valid_profile_sha256(sha)
        {
            return Err(invalid_schema(
                "profile_sha256",
                "profile_sha256 must match sha256:<64 lowercase hex chars>",
            ));
        }

        // Status is already validated to be frozen if we get here
    } else if matches!(profile.status, ProfileStatus::Draft)
        && (profile.profile_id.is_some()
            || profile.profile_version.is_some()
            || profile.profile_family.is_some()
            || profile.profile_sha256.is_some())
    {
        return Err(invalid_schema(
            "status",
            "draft profile must not set frozen-only identity fields",
        ));
    }

    if let Some(hashing) = profile.hashing
        && !matches!(hashing.algorithm, HashAlgorithm::Sha256)
    {
        return Err(invalid_schema("hashing.algorithm", "must be sha256"));
    }

    Ok(())
}

pub fn is_valid_profile_family(family: &str) -> bool {
    let mut segments = family.split('.');
    let Some(first) = segments.next() else {
        return false;
    };

    if !is_first_family_segment(first) {
        return false;
    }

    segments.all(is_family_segment)
}

pub fn is_valid_profile_sha256(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };

    hex.len() == 64
        && hex
            .chars()
            .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch))
}

fn is_first_family_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    first.is_ascii_lowercase() && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
}

fn is_family_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    first.is_ascii_lowercase()
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

fn map_yaml_error(error: serde_yaml::Error) -> RefusalPayload {
    let message = error.to_string();

    if let Some(field) = extract_missing_field(&message) {
        return missing_field(field);
    }

    invalid_schema("yaml", format!("invalid profile YAML: {message}"))
}

fn extract_missing_field(message: &str) -> Option<&str> {
    let (_, after_prefix) = message.split_once("missing field `")?;
    let (field, _) = after_prefix.split_once('`')?;
    Some(field)
}

fn missing_field(field: &str) -> RefusalPayload {
    RefusalPayload::missing_field(field)
}

fn invalid_schema(field: impl Into<String>, error: impl Into<String>) -> RefusalPayload {
    RefusalPayload::invalid_schema_single(field, error)
}
