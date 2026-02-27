use std::fs;
use std::fs::File;

use serde::Serialize;
use serde_json::{Value, json};

use crate::cli::args::{DatasetFormat, DraftInitArgs, SuggestKeyArgs};
use crate::refusal::RefusalPayload;
use crate::schema::{
    Equivalence, Profile, ProfileFormat, ProfileStatus, ValidationMode, validate_profile,
};
use crate::stats::suggest_key;

pub fn run(args: &DraftInitArgs, _no_witness: bool) -> Result<Value, RefusalPayload> {
    let file = File::open(&args.dataset).map_err(|error| {
        RefusalPayload::io(args.dataset.display().to_string(), error.to_string())
    })?;
    let mut reader = csv::Reader::from_reader(file);

    let headers = reader
        .headers()
        .map_err(|error| {
            RefusalPayload::csv_parse(args.dataset.display().to_string(), error.to_string())
        })?
        .clone();
    if headers.is_empty() {
        return Err(RefusalPayload::empty_with_reason(
            args.dataset.display().to_string(),
            "no header row",
        ));
    }

    let include_columns = headers
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();
    let key = resolve_key(args)?;

    let profile = Profile {
        schema_version: 1,
        profile_id: None,
        profile_version: None,
        profile_family: None,
        profile_sha256: None,
        frozen: None,
        status: ProfileStatus::Draft,
        format: resolve_profile_format(&args.format)?,
        hashing: None,
        equivalence: Some(Equivalence {
            order: None,
            float_decimals: Some(6),
            trim_strings: Some(true),
        }),
        key,
        include_columns,
    };
    validate_profile(&profile, ValidationMode::Validate)?;

    let yaml = render_yaml(&profile)?;
    fs::write(&args.out, yaml)
        .map_err(|error| RefusalPayload::io(args.out.display().to_string(), error.to_string()))?;

    Ok(json!({
        "path": args.out.display().to_string()
    }))
}

fn resolve_key(args: &DraftInitArgs) -> Result<Vec<String>, RefusalPayload> {
    match args.key.as_deref() {
        None => Ok(Vec::new()),
        Some("auto") => resolve_auto_key(args),
        Some(explicit_key) => Ok(vec![explicit_key.to_string()]),
    }
}

fn resolve_auto_key(args: &DraftInitArgs) -> Result<Vec<String>, RefusalPayload> {
    let suggest_args = SuggestKeyArgs {
        dataset: args.dataset.clone(),
        top: 1,
    };
    let suggest_output = suggest_key::run(&suggest_args, true)?;

    let candidate = suggest_output
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first());

    let Some(candidate) = candidate else {
        eprintln!("Warning: no viable key candidate found; using key: []");
        return Ok(Vec::new());
    };

    let viable = candidate
        .get("viable")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let column = candidate
        .get("column")
        .and_then(Value::as_str)
        .unwrap_or("");

    if viable && !column.is_empty() {
        Ok(vec![column.to_string()])
    } else {
        eprintln!("Warning: no viable key candidate found; using key: []");
        Ok(Vec::new())
    }
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
