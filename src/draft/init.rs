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
    let pre_parse = load_pre_parse_from_peek(args)?;
    let headers = if let Some(pre_parse) = pre_parse.as_ref() {
        csv::StringRecord::from(crate::slice::headers_from_pre_parse(
            &args.dataset,
            pre_parse,
        )?)
    } else {
        let file = File::open(&args.dataset).map_err(|error| {
            RefusalPayload::io(args.dataset.display().to_string(), error.to_string())
        })?;
        let mut reader = csv::Reader::from_reader(file);
        reader
            .headers()
            .map_err(|error| {
                RefusalPayload::csv_parse(args.dataset.display().to_string(), error.to_string())
            })?
            .clone()
    };
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
        fingerprint_ref: None,
        pre_parse,
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

fn load_pre_parse_from_peek(
    args: &DraftInitArgs,
) -> Result<Option<crate::schema::PreParse>, RefusalPayload> {
    let Some(path) = args.from_peek.as_deref() else {
        return Ok(None);
    };
    let content = fs::read_to_string(path)
        .map_err(|error| RefusalPayload::io(path.display().to_string(), error.to_string()))?;
    let value: Value = serde_json::from_str(&content).map_err(|error| {
        RefusalPayload::invalid_schema_single(
            "from_peek",
            format!("peek JSON could not be parsed: {error}"),
        )
    })?;
    let suggestion = value
        .pointer("/result/suggestions/profile_pre_parse")
        .ok_or_else(|| {
            RefusalPayload::invalid_schema_single(
                "from_peek",
                "peek JSON did not include result.suggestions.profile_pre_parse; rerun fingerprint peek --suggest",
            )
        })?;
    let mode = suggestion
        .get("mode")
        .and_then(Value::as_str)
        .ok_or_else(|| RefusalPayload::missing_field("from_peek.mode"))?;
    let mode = match mode {
        "preamble_skip" => crate::schema::SliceMode::PreambleSkip,
        "multi_row_header" => crate::schema::SliceMode::MultiRowHeader,
        "preamble_with_units" => crate::schema::SliceMode::PreambleWithUnits,
        other => {
            return Err(RefusalPayload::invalid_schema_single(
                "from_peek.mode",
                format!("unsupported peek mode '{other}'"),
            ));
        }
    };

    let slice = crate::schema::SliceDirectives {
        mode,
        skip_rows: read_usize(suggestion, "skip_rows"),
        header_at_row: read_usize(suggestion, "header_at_row"),
        header_rows: Vec::new(),
        header_merge: None,
        data_starts_at: read_usize(suggestion, "data_starts_at"),
        delimiter: None,
        encoding: None,
        preamble_capture: Some(true),
        unit_rows_capture: Some(true),
        unit_rows: suggestion
            .get("unit_rows")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(Value::as_u64)
                    .filter_map(|value| usize::try_from(value).ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    };

    let expected_shape =
        value
            .pointer("/result/summary")
            .map(|summary| crate::schema::ExpectedShape {
                modal_column_count: read_usize(summary, "modal_column_count"),
                first_data_row: read_usize(summary, "data_starts_at"),
                header_rows_pattern: Vec::new(),
            });

    Ok(Some(crate::schema::PreParse {
        expected_shape,
        slice,
    }))
}

fn read_usize(value: &Value, key: &str) -> Option<usize> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|entry| usize::try_from(entry).ok())
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
        pre_parse: profile.pre_parse.as_ref(),
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
    pre_parse: Option<&'a crate::schema::PreParse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    equivalence: Option<&'a Equivalence>,
    key: &'a [String],
    include_columns: &'a [String],
}
