use std::fs;

use serde::Serialize;
use serde_json::{Value, json};

use crate::cli::args::{DatasetFormat, DraftNewArgs};
use crate::refusal::RefusalPayload;
use crate::schema::{
    Equivalence, Profile, ProfileFormat, ProfileStatus, ValidationMode, validate_profile,
};

pub fn run(args: &DraftNewArgs, _no_witness: bool) -> Result<Value, RefusalPayload> {
    let profile = build_draft_template(&args.format)?;
    validate_profile(&profile, ValidationMode::Validate)?;

    let yaml = render_yaml(&profile)?;
    fs::write(&args.out, yaml)
        .map_err(|error| RefusalPayload::io(args.out.display().to_string(), error.to_string()))?;

    Ok(json!({
        "path": args.out.display().to_string()
    }))
}

fn build_draft_template(format: &DatasetFormat) -> Result<Profile, RefusalPayload> {
    let resolved_format = resolve_profile_format(format)?;

    Ok(Profile {
        schema_version: 1,
        profile_id: None,
        profile_version: None,
        profile_family: None,
        profile_sha256: None,
        frozen: None,
        status: ProfileStatus::Draft,
        format: resolved_format,
        hashing: None,
        equivalence: Some(Equivalence {
            order: None,
            float_decimals: Some(6),
            trim_strings: Some(true),
        }),
        key: Vec::new(),
        include_columns: Vec::new(),
    })
}

fn resolve_profile_format(format: &DatasetFormat) -> Result<ProfileFormat, RefusalPayload> {
    match format {
        DatasetFormat::Csv => Ok(ProfileFormat::Csv),
    }
}

fn render_yaml(profile: &Profile) -> Result<String, RefusalPayload> {
    let template = DraftTemplate {
        schema_version: profile.schema_version,
        status: profile.status,
        format: profile.format,
        equivalence: profile.equivalence.as_ref(),
        key: profile.key.as_slice(),
        include_columns: profile.include_columns.as_slice(),
    };

    let rendered = serde_yaml::to_string(&template).map_err(|error| {
        RefusalPayload::invalid_schema_single(
            "draft",
            format!("failed to serialize draft profile: {error}"),
        )
    })?;

    let without_marker = rendered.strip_prefix("---\n").unwrap_or(&rendered);
    let trimmed = without_marker.trim_end_matches('\n');
    Ok(format!("{trimmed}\n"))
}

#[derive(Debug, Serialize)]
struct DraftTemplate<'a> {
    schema_version: u32,
    status: ProfileStatus,
    format: ProfileFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    equivalence: Option<&'a Equivalence>,
    key: &'a [String],
    include_columns: &'a [String],
}
