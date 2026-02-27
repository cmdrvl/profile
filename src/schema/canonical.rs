use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::refusal::RefusalPayload;
use crate::schema::profile::{Equivalence, Hashing, Profile, ProfileFormat, ProfileStatus};
use crate::schema::validate::{ValidationMode, validate_profile};

#[derive(Debug, Clone, Serialize)]
struct CanonicalProfile<'a> {
    schema_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile_version: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile_family: Option<&'a str>,
    status: ProfileStatus,
    format: ProfileFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    hashing: Option<CanonicalHashing<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    equivalence: Option<CanonicalEquivalence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<&'a [String]>,
    include_columns: &'a [String],
}

#[derive(Debug, Clone, Copy, Serialize)]
struct CanonicalHashing<'a> {
    algorithm: &'a crate::schema::profile::HashAlgorithm,
}

#[derive(Debug, Clone, Serialize)]
struct CanonicalEquivalence {
    #[serde(skip_serializing_if = "Option::is_none")]
    order: Option<crate::schema::profile::EquivalenceOrder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    float_decimals: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trim_strings: Option<bool>,
}

pub fn canonical_yaml(profile: &Profile) -> Result<String, RefusalPayload> {
    validate_profile(profile, ValidationMode::Freeze)?;

    let canonical = CanonicalProfile::from(profile);
    let serialized = serde_yaml::to_string(&canonical).map_err(|error| RefusalPayload {
        code: "E_INVALID_SCHEMA".to_string(),
        detail: format!("failed to canonicalize profile: {error}"),
    })?;

    Ok(normalize_yaml(serialized))
}

pub fn canonical_bytes(profile: &Profile) -> Result<Vec<u8>, RefusalPayload> {
    Ok(canonical_yaml(profile)?.into_bytes())
}

pub fn compute_profile_sha256(canonical_yaml: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_yaml.as_bytes());
    let digest = hasher.finalize();
    format!("sha256:{digest:x}")
}

impl<'a> From<&'a Profile> for CanonicalProfile<'a> {
    fn from(profile: &'a Profile) -> Self {
        Self {
            schema_version: profile.schema_version,
            profile_id: profile.profile_id.as_deref(),
            profile_version: profile.profile_version,
            profile_family: profile.profile_family.as_deref(),
            status: profile.status,
            format: profile.format,
            hashing: profile.hashing.as_ref().map(CanonicalHashing::from),
            equivalence: profile.equivalence.as_ref().map(CanonicalEquivalence::from),
            key: (!profile.key.is_empty()).then_some(profile.key.as_slice()),
            include_columns: &profile.include_columns,
        }
    }
}

impl<'a> From<&'a Hashing> for CanonicalHashing<'a> {
    fn from(value: &'a Hashing) -> Self {
        Self {
            algorithm: &value.algorithm,
        }
    }
}

impl From<&Equivalence> for CanonicalEquivalence {
    fn from(value: &Equivalence) -> Self {
        Self {
            order: value.order,
            float_decimals: value.float_decimals,
            trim_strings: value.trim_strings,
        }
    }
}

fn normalize_yaml(serialized: String) -> String {
    let without_marker = serialized
        .strip_prefix("---\n")
        .map_or(serialized.as_str(), |trimmed| trimmed);

    let trimmed = without_marker.trim_end_matches('\n');
    format!("{trimmed}\n")
}
