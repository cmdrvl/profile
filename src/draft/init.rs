use std::fs;
use std::fs::File;

use serde::Serialize;
use serde_json::{Value, json};

use crate::cli::args::{DatasetFormat, DraftInitArgs, SuggestKeyArgs};
use crate::refusal::RefusalPayload;
use crate::schema::{
    Equivalence, Profile, ProfileFormat, ProfileStatus, ValidationMode,
    canonicalize_header_sequence, canonicalize_profile_column, load_column_registry_aliases,
    validate_profile,
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

    let column_aliases = args
        .column_registry
        .as_deref()
        .map(load_column_registry_aliases)
        .transpose()?;
    let include_columns = canonicalize_header_sequence(&headers, column_aliases.as_ref());
    let key = resolve_key(args, column_aliases.as_ref())?;

    let profile = Profile {
        schema_version: 1,
        profile_id: None,
        profile_version: None,
        profile_family: None,
        profile_sha256: None,
        frozen: None,
        status: ProfileStatus::Draft,
        format: resolve_profile_format(&args.format)?,
        column_registry: args
            .column_registry
            .as_ref()
            .map(|path| path.display().to_string()),
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

fn resolve_key(
    args: &DraftInitArgs,
    column_aliases: Option<&std::collections::HashMap<String, String>>,
) -> Result<Vec<String>, RefusalPayload> {
    match args.key.as_deref() {
        None => Ok(Vec::new()),
        Some("auto") => resolve_auto_key(args, column_aliases),
        Some(explicit_key) => Ok(vec![canonicalize_profile_column(
            explicit_key,
            column_aliases,
        )]),
    }
}

fn resolve_auto_key(
    args: &DraftInitArgs,
    column_aliases: Option<&std::collections::HashMap<String, String>>,
) -> Result<Vec<String>, RefusalPayload> {
    let suggest_args = SuggestKeyArgs {
        dataset: args.dataset.clone(),
        top: 1,
    };
    let suggest_output = suggest_key::run(&suggest_args, true)?;

    let candidate = suggest_output
        .result
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
        Ok(vec![canonicalize_profile_column(column, column_aliases)])
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
        column_registry: profile.column_registry.as_deref(),
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
    column_registry: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    equivalence: Option<&'a Equivalence>,
    key: &'a [String],
    include_columns: &'a [String],
}
