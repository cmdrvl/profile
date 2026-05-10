use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::cli::args::{HeaderMergeStrategyArg, SliceArgs, SliceModeArg};
use crate::output::json::{CommandOutput, ProfileRef};
use crate::refusal::RefusalPayload;
use crate::resolve::resolver::resolve_profile;
use crate::schema::{
    HeaderMerge, HeaderMergeStrategy, PreParse, Profile, SliceDirectives, SliceMode,
    ValidationMode, parse_profile_yaml, validate_profile,
};
use crate::witness::append::append_for_command;

pub fn headers_from_pre_parse(
    path: &Path,
    pre_parse: &PreParse,
) -> Result<Vec<String>, RefusalPayload> {
    let delimiter = resolve_delimiter(&pre_parse.slice)?;
    let rows = read_physical_rows(path, delimiter)?;
    let plan = build_plan(&pre_parse.slice)?;
    let slice = build_slice(&rows, &plan, &pre_parse.slice)?;
    Ok(slice.headers)
}

pub fn run(
    args: &SliceArgs,
    no_witness: bool,
    explicit: bool,
    json_output: bool,
) -> Result<CommandOutput, RefusalPayload> {
    let resolved_profile = resolve_slice_profile(args)?;
    let profile = resolved_profile.as_ref().map(|resolved| &resolved.profile);
    let profile_path = resolved_profile
        .as_ref()
        .map(|resolved| resolved.path.clone());
    let (directives, mut warnings) = effective_directives(args, profile)?;
    validate_directives(&directives)?;

    let delimiter = resolve_delimiter(&directives)?;
    let rows = read_physical_rows(&args.file, delimiter)?;
    let plan = build_plan(&directives)?;
    let slice = build_slice(&rows, &plan, &directives)?;
    if let Some(warning) = modal_column_count_warning(profile, slice.headers.len()) {
        warnings.push(warning);
    }
    let csv_bytes = render_csv(&slice.headers, &slice.data_rows)?;
    let output_hash = format!("blake3:{}", blake3::hash(&csv_bytes).to_hex());

    if let Some(out) = args.out.as_deref() {
        fs::write(out, &csv_bytes)
            .map_err(|error| RefusalPayload::io(out.display().to_string(), error.to_string()))?;
    }

    if let Some(manifest_path) = args.emit_manifest.as_deref() {
        let manifest = build_manifest(
            args,
            profile,
            &directives,
            &plan,
            &slice,
            &rows,
            &output_hash,
        );
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| {
            RefusalPayload::invalid_schema_single(
                "manifest",
                format!("failed to serialize slice manifest: {error}"),
            )
        })?;
        fs::write(manifest_path, manifest_bytes).map_err(|error| {
            RefusalPayload::io(manifest_path.display().to_string(), error.to_string())
        })?;
    }

    let mut result = json!({
        "input_path": args.file.display().to_string(),
        "output_path": args.out.as_ref().map(|path| path.display().to_string()),
        "manifest_path": args.emit_manifest.as_ref().map(|path| path.display().to_string()),
        "profile_id": profile.and_then(|profile| profile.profile_id.clone()),
        "fingerprint_ref": profile.and_then(|profile| profile.fingerprint_ref.clone()),
        "mode": directives.mode.as_str(),
        "directives": directive_summary(&directives),
        "rows": {
            "input_physical_rows": rows.len(),
            "header_rows": plan.header_rows,
            "unit_rows": plan.unit_rows,
            "data_starts_at": plan.data_starts_at,
            "output_data_rows": slice.data_rows.len()
        },
        "columns": slice.headers,
        "output_hash": output_hash
    });
    if !warnings.is_empty() {
        result["warnings"] = json!(warnings);
        if !json_output {
            emit_slice_warnings(&warnings);
        }
    }

    if explicit || (!json_output && args.out.is_none()) {
        result["slice_csv"] =
            Value::String(String::from_utf8(csv_bytes.clone()).map_err(|error| {
                RefusalPayload::invalid_schema_single(
                    "slice",
                    format!("slice output was not valid UTF-8: {error}"),
                )
            })?);
    }

    let mut input_paths = vec![args.file.clone()];
    if let Some(profile_path) = profile_path.as_ref() {
        input_paths.push(profile_path.clone());
    }
    let witness_result = redacted_witness_result(&result);
    let witness_id = append_for_command(
        "slice",
        &witness_result,
        input_paths,
        json!({ "directives": directive_summary(&directives) }),
        no_witness,
    );

    Ok(CommandOutput::success(result)
        .with_profile_ref(profile.and_then(ProfileRef::from_profile))
        .with_witness_id(witness_id))
}

#[derive(Debug, Clone)]
struct SliceProfile {
    path: PathBuf,
    profile: Profile,
}

fn resolve_slice_profile(args: &SliceArgs) -> Result<Option<SliceProfile>, RefusalPayload> {
    match (args.profile.as_deref(), args.profile_path.as_deref()) {
        (Some(_), Some(_)) => Err(RefusalPayload::invalid_schema_single(
            "profile",
            "use either --profile or --profile-path, not both",
        )),
        (Some(profile_ref), None) => {
            let resolved = resolve_profile(profile_ref)?;
            Ok(Some(SliceProfile {
                path: resolved.path,
                profile: resolved.profile,
            }))
        }
        (None, Some(path)) => {
            let content = fs::read_to_string(path).map_err(|error| {
                RefusalPayload::io(path.display().to_string(), error.to_string())
            })?;
            let profile = parse_profile_yaml(&content)?;
            validate_profile(&profile, ValidationMode::Validate)?;
            Ok(Some(SliceProfile {
                path: path.to_path_buf(),
                profile,
            }))
        }
        (None, None) => Ok(None),
    }
}

fn effective_directives(
    args: &SliceArgs,
    profile: Option<&Profile>,
) -> Result<(SliceDirectives, Vec<String>), RefusalPayload> {
    let mut directives = profile
        .and_then(|profile| profile.pre_parse.as_ref())
        .map(|pre_parse| pre_parse.slice.clone())
        .unwrap_or_else(|| SliceDirectives {
            mode: infer_mode_from_args(args),
            skip_rows: None,
            header_at_row: None,
            header_rows: Vec::new(),
            header_merge: None,
            data_starts_at: None,
            delimiter: None,
            encoding: None,
            preamble_capture: Some(true),
            unit_rows_capture: Some(true),
            unit_rows: Vec::new(),
        });
    let mut overridden_flags = Vec::new();

    if let Some(mode) = args.mode {
        directives.mode = mode.into();
        overridden_flags.push("--mode");
    }
    if let Some(skip_rows) = args.skip_rows {
        directives.skip_rows = Some(skip_rows);
        overridden_flags.push("--skip-rows");
    }
    if let Some(header_at_row) = args.header_at_row {
        directives.header_at_row = Some(header_at_row);
        overridden_flags.push("--header-at-row");
    }
    if let Some(header_rows) = args.header_rows.as_deref() {
        directives.header_rows = parse_row_list("header_rows", header_rows)?;
        overridden_flags.push("--header-rows");
    }
    if args.header_merge.is_some() {
        directives.header_merge = Some(HeaderMerge {
            strategy: args
                .header_merge
                .unwrap_or(HeaderMergeStrategyArg::FfillConcat)
                .into(),
            separator: Some(args.header_merge_sep.clone()),
            empty_placeholder: None,
        });
        overridden_flags.push("--header-merge");
    }
    if let Some(unit_rows) = args.unit_rows.as_deref() {
        directives.unit_rows = parse_row_list("unit_rows", unit_rows)?;
        overridden_flags.push("--unit-rows");
    }
    if let Some(data_starts_at) = args.data_starts_at {
        directives.data_starts_at = Some(data_starts_at);
        overridden_flags.push("--data-starts-at");
    }
    if let Some(delimiter) = args.delimiter.as_ref() {
        directives.delimiter = Some(delimiter.clone());
        overridden_flags.push("--delimiter");
    }
    if let Some(encoding) = args.encoding.as_ref() {
        directives.encoding = Some(encoding.clone());
        overridden_flags.push("--encoding");
    }

    if profile.is_some() && directives.header_rows.is_empty() && directives.header_at_row.is_none()
    {
        return Err(RefusalPayload::invalid_schema_single(
            "pre_parse",
            "profile does not contain usable pre_parse slice directives",
        ));
    }

    let warnings = if profile.is_some() && !overridden_flags.is_empty() {
        vec![format!(
            "profile pre_parse directives were overridden by CLI flags: {}",
            overridden_flags.join(", ")
        )]
    } else {
        Vec::new()
    };

    Ok((directives, warnings))
}

fn infer_mode_from_args(args: &SliceArgs) -> SliceMode {
    if args.header_rows.is_some() {
        SliceMode::MultiRowHeader
    } else if args.unit_rows.is_some() {
        SliceMode::PreambleWithUnits
    } else {
        SliceMode::PreambleSkip
    }
}

impl From<SliceModeArg> for SliceMode {
    fn from(value: SliceModeArg) -> Self {
        match value {
            SliceModeArg::PreambleSkip => Self::PreambleSkip,
            SliceModeArg::MultiRowHeader => Self::MultiRowHeader,
            SliceModeArg::PreambleWithUnits => Self::PreambleWithUnits,
        }
    }
}

impl From<HeaderMergeStrategyArg> for HeaderMergeStrategy {
    fn from(value: HeaderMergeStrategyArg) -> Self {
        match value {
            HeaderMergeStrategyArg::FfillConcat => Self::FfillConcat,
            HeaderMergeStrategyArg::ConcatOnly => Self::ConcatOnly,
            HeaderMergeStrategyArg::FirstNonEmpty => Self::FirstNonEmpty,
        }
    }
}

fn parse_row_list(field: &str, value: &str) -> Result<Vec<usize>, RefusalPayload> {
    let mut rows = Vec::new();
    for part in value.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let row = trimmed.parse::<usize>().map_err(|error| {
            RefusalPayload::invalid_schema_single(
                format!("pre_parse.slice.{field}"),
                format!("row '{trimmed}' is not an integer: {error}"),
            )
        })?;
        rows.push(row);
    }
    Ok(rows)
}

fn validate_directives(directives: &SliceDirectives) -> Result<(), RefusalPayload> {
    let profile = Profile {
        schema_version: 1,
        profile_id: None,
        profile_version: None,
        profile_family: None,
        profile_sha256: None,
        frozen: None,
        status: crate::schema::ProfileStatus::Draft,
        format: crate::schema::ProfileFormat::Csv,
        column_registry: None,
        fingerprint_ref: None,
        pre_parse: Some(PreParse {
            expected_shape: None,
            slice: directives.clone(),
        }),
        hashing: None,
        equivalence: None,
        key: Vec::new(),
        include_columns: vec!["slice_placeholder".to_owned()],
    };
    validate_profile(&profile, ValidationMode::Validate)
}

fn resolve_delimiter(directives: &SliceDirectives) -> Result<u8, RefusalPayload> {
    match directives.delimiter.as_deref() {
        None => Ok(b','),
        Some("\\t") | Some("tab") => Ok(b'\t'),
        Some(value) if value.chars().count() == 1 => Ok(value.as_bytes()[0]),
        Some(_) => Err(RefusalPayload::invalid_schema_single(
            "pre_parse.slice.delimiter",
            "delimiter must be one character, \\t, or tab",
        )),
    }
}

fn read_physical_rows(path: &Path, delimiter: u8) -> Result<Vec<Vec<String>>, RefusalPayload> {
    let file = File::open(path)
        .map_err(|error| RefusalPayload::io(path.display().to_string(), error.to_string()))?;
    let mut buffered = BufReader::new(file);
    let mut rows = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        let read = buffered
            .read_line(&mut line)
            .map_err(|error| RefusalPayload::io(path.display().to_string(), error.to_string()))?;
        if read == 0 {
            break;
        }
        while line.ends_with('\n') || line.ends_with('\r') {
            line.pop();
        }
        if line.trim().is_empty() {
            rows.push(Vec::new());
            continue;
        }
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .delimiter(delimiter)
            .from_reader(line.as_bytes());
        let record = reader
            .records()
            .next()
            .transpose()
            .map_err(|error| {
                RefusalPayload::csv_parse(path.display().to_string(), error.to_string())
            })?
            .ok_or_else(|| RefusalPayload::empty(path.display().to_string()))?;
        rows.push(record.iter().map(ToOwned::to_owned).collect());
    }
    if rows.is_empty() {
        return Err(RefusalPayload::empty_with_reason(
            path.display().to_string(),
            "no rows",
        ));
    }
    Ok(rows)
}

fn modal_column_count_warning(profile: Option<&Profile>, output_columns: usize) -> Option<String> {
    let expected = profile
        .and_then(|profile| profile.pre_parse.as_ref())
        .and_then(|pre_parse| pre_parse.expected_shape.as_ref())
        .and_then(|expected_shape| expected_shape.modal_column_count)?;
    (expected != output_columns).then(|| {
        format!(
            "expected_shape.modal_column_count is {} but slice produced {} columns",
            expected, output_columns
        )
    })
}

fn emit_slice_warnings(warnings: &[String]) {
    for warning in warnings {
        eprintln!("Warning: {warning}");
    }
}

#[derive(Debug, Clone)]
struct SlicePlan {
    header_rows: Vec<usize>,
    unit_rows: Vec<usize>,
    data_starts_at: usize,
}

fn build_plan(directives: &SliceDirectives) -> Result<SlicePlan, RefusalPayload> {
    let header_rows = match directives.mode {
        SliceMode::PreambleSkip | SliceMode::PreambleWithUnits => {
            vec![directives.header_at_row.unwrap_or_else(|| {
                directives
                    .skip_rows
                    .map(|skip_rows| skip_rows + 1)
                    .unwrap_or(1)
            })]
        }
        SliceMode::MultiRowHeader => directives.header_rows.clone(),
    };
    let last_structural_row = header_rows
        .iter()
        .copied()
        .chain(directives.unit_rows.iter().copied())
        .max()
        .unwrap_or(1);
    let data_starts_at = directives.data_starts_at.unwrap_or(last_structural_row + 1);
    Ok(SlicePlan {
        header_rows,
        unit_rows: directives.unit_rows.clone(),
        data_starts_at,
    })
}

#[derive(Debug, Clone)]
struct SliceOutput {
    headers: Vec<String>,
    data_rows: Vec<Vec<String>>,
}

fn build_slice(
    rows: &[Vec<String>],
    plan: &SlicePlan,
    directives: &SliceDirectives,
) -> Result<SliceOutput, RefusalPayload> {
    let header_source = plan
        .header_rows
        .iter()
        .map(|row| physical_row(rows, *row))
        .collect::<Result<Vec<_>, _>>()?;
    let data_rows = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let row_number = index + 1;
            (row_number >= plan.data_starts_at && !row.iter().all(|cell| cell.trim().is_empty()))
                .then_some(row.clone())
        })
        .collect::<Vec<_>>();
    if data_rows.is_empty() {
        return Err(RefusalPayload::empty_with_reason(
            "slice",
            "no data rows after slice directives applied",
        ));
    }
    let width = header_source
        .iter()
        .chain(data_rows.iter())
        .map(Vec::len)
        .max()
        .unwrap_or(0);
    if width == 0 {
        return Err(RefusalPayload::empty_with_reason(
            "slice",
            "no header columns found",
        ));
    }
    let headers = merge_headers(&header_source, width, directives);
    let data_rows = data_rows
        .into_iter()
        .map(|row| pad_row(row, width))
        .collect::<Vec<_>>();
    Ok(SliceOutput { headers, data_rows })
}

fn physical_row(rows: &[Vec<String>], row_number: usize) -> Result<Vec<String>, RefusalPayload> {
    rows.get(row_number.saturating_sub(1))
        .cloned()
        .ok_or_else(|| {
            RefusalPayload::invalid_schema_single(
                "pre_parse.slice",
                format!("row {row_number} is outside the input"),
            )
        })
}

fn merge_headers(
    header_rows: &[Vec<String>],
    width: usize,
    directives: &SliceDirectives,
) -> Vec<String> {
    let merge = directives.header_merge.as_ref();
    let strategy = merge
        .map(|merge| merge.strategy)
        .unwrap_or(HeaderMergeStrategy::FfillConcat);
    let separator = merge
        .and_then(|merge| merge.separator.as_deref())
        .unwrap_or(".");
    let empty_placeholder = merge
        .and_then(|merge| merge.empty_placeholder.as_deref())
        .unwrap_or("column");
    let mut normalized = header_rows
        .iter()
        .map(|row| pad_row(row.clone(), width))
        .collect::<Vec<_>>();

    if matches!(strategy, HeaderMergeStrategy::FfillConcat) {
        for row in &mut normalized {
            let mut last = String::new();
            for cell in row {
                if cell.trim().is_empty() {
                    *cell = last.clone();
                } else {
                    last = cell.trim().to_owned();
                }
            }
        }
    }

    (0..width)
        .map(|column| {
            let parts = normalized
                .iter()
                .filter_map(|row| {
                    let value = row.get(column).map(|cell| cell.trim()).unwrap_or("");
                    (!value.is_empty()).then_some(value.to_owned())
                })
                .collect::<Vec<_>>();
            let name = match strategy {
                HeaderMergeStrategy::FfillConcat | HeaderMergeStrategy::ConcatOnly => {
                    parts.join(separator)
                }
                HeaderMergeStrategy::FirstNonEmpty => parts.first().cloned().unwrap_or_default(),
            };
            if name.trim().is_empty() {
                format!("{empty_placeholder}_{}", column + 1)
            } else {
                name
            }
        })
        .collect()
}

fn pad_row(mut row: Vec<String>, width: usize) -> Vec<String> {
    row.resize(width, String::new());
    row
}

fn render_csv(headers: &[String], rows: &[Vec<String>]) -> Result<Vec<u8>, RefusalPayload> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record(headers).map_err(|error| {
        RefusalPayload::csv_parse("slice", format!("failed to write header: {error}"))
    })?;
    for row in rows {
        writer.write_record(row).map_err(|error| {
            RefusalPayload::csv_parse("slice", format!("failed to write row: {error}"))
        })?;
    }
    writer.flush().map_err(|error| {
        RefusalPayload::io("slice", format!("failed to flush output CSV: {error}"))
    })?;
    writer.into_inner().map_err(|error| {
        RefusalPayload::io("slice", format!("failed to finalize output CSV: {error}"))
    })
}

fn build_manifest(
    args: &SliceArgs,
    profile: Option<&Profile>,
    directives: &SliceDirectives,
    plan: &SlicePlan,
    slice: &SliceOutput,
    rows: &[Vec<String>],
    output_hash: &str,
) -> Value {
    let preamble_rows = if directives.preamble_capture.unwrap_or(true) {
        rows.iter()
            .take(
                plan.header_rows
                    .first()
                    .copied()
                    .unwrap_or(1)
                    .saturating_sub(1),
            )
            .cloned()
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let unit_rows = if directives.unit_rows_capture.unwrap_or(true) {
        plan.unit_rows
            .iter()
            .filter_map(|row| rows.get(row.saturating_sub(1)).cloned())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    json!({
        "schema": "profile.slice_manifest.v1",
        "input_path": args.file.display().to_string(),
        "profile_id": profile.and_then(|profile| profile.profile_id.clone()),
        "fingerprint_ref": profile.and_then(|profile| profile.fingerprint_ref.clone()),
        "directives": directive_summary(directives),
        "header_rows": &plan.header_rows,
        "unit_rows": &plan.unit_rows,
        "data_starts_at": plan.data_starts_at,
        "columns": &slice.headers,
        "output_data_rows": slice.data_rows.len(),
        "output_hash": output_hash,
        "preamble_rows": preamble_rows,
        "unit_row_values": unit_rows
    })
}

fn directive_summary(directives: &SliceDirectives) -> Value {
    json!({
        "mode": directives.mode.as_str(),
        "skip_rows": directives.skip_rows,
        "header_at_row": directives.header_at_row,
        "header_rows": &directives.header_rows,
        "header_merge": directives.header_merge.as_ref().map(|merge| json!({
            "strategy": merge.strategy.as_str(),
            "separator": merge.separator.as_deref()
        })),
        "data_starts_at": directives.data_starts_at,
        "delimiter": directives.delimiter.as_deref(),
        "encoding": directives.encoding.as_deref(),
        "unit_rows": &directives.unit_rows,
        "preamble_capture": directives.preamble_capture,
        "unit_rows_capture": directives.unit_rows_capture
    })
}

fn redacted_witness_result(result: &Value) -> Value {
    let mut result = result.clone();
    if let Some(object) = result.as_object_mut() {
        object.remove("slice_csv");
    }
    result
}
