use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Value, json};

use crate::cli::args::{HeaderMergeStrategyArg, SliceArgs, SliceModeArg};
use crate::output::json::{CommandOutput, ProfileRef};
use crate::refusal::RefusalPayload;
use crate::resolve::resolver::resolve_profile;
use crate::schema::{
    HeaderMerge, HeaderMergeStrategy, PreParse, Profile, SliceDirectives, SliceMode,
    ValidationMode, load_column_registry_aliases, parse_profile_yaml, registry_content_hash,
    resolve_registry_path, validate_frozen_registry_hash, validate_profile,
};
use crate::witness::append::append_for_command;

const CANONICALIZER_VERSION: &str = "profile.canonical_csv_headers.v1";

pub fn headers_from_pre_parse(
    path: &Path,
    pre_parse: &PreParse,
) -> Result<Vec<String>, RefusalPayload> {
    let delimiter = resolve_delimiter(&pre_parse.slice)?;
    let source_encoding = resolve_source_encoding(&pre_parse.slice)?;
    let bytes = read_source_bytes(path)?;
    let rows = parse_physical_rows(path, &bytes, delimiter, source_encoding)?;
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
    let source_encoding = resolve_source_encoding(&directives)?;
    let source_bytes = read_source_bytes(&args.file)?;
    let input_hash = format!("blake3:{}", blake3::hash(&source_bytes).to_hex());
    let rows = parse_physical_rows(&args.file, &source_bytes, delimiter, source_encoding)?;
    let plan = build_plan(&directives)?;
    let mut slice = build_slice(&rows, &plan, &directives)?;
    let canonicalization =
        canonicalize_slice_headers(profile_path.as_deref(), profile, &slice.headers)?;
    if let Some(canonicalization) = canonicalization.as_ref() {
        slice.headers = canonicalization.headers.clone();
    }
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
        let manifest = build_manifest(SliceManifestInputs {
            args,
            profile,
            directives: &directives,
            plan: &plan,
            slice: &slice,
            rows: &rows,
            input_hash: &input_hash,
            output_hash: &output_hash,
            source_encoding,
            canonicalization: canonicalization.as_ref(),
        });
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
        "source_encoding": source_encoding.label(),
        "mode": directives.mode.as_str(),
        "directives": directive_summary(&directives),
        "canonical_headers": canonical_header_summary(canonicalization.as_ref()),
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
            validate_frozen_registry_hash(&resolved.path, &resolved.profile)?;
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
            validate_frozen_registry_hash(path, &profile)?;
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
    let profile_directives = profile
        .and_then(|profile| profile.pre_parse.as_ref())
        .map(|pre_parse| pre_parse.slice.clone());
    let profile_had_directives = profile_directives.is_some();
    let mut directives = profile_directives.unwrap_or_else(|| SliceDirectives {
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

    let warnings = if profile_had_directives && !overridden_flags.is_empty() {
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
        column_registry_hash: None,
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

#[derive(Debug, Clone, Copy)]
enum SourceEncoding {
    Utf8,
    Windows1252,
    Latin1,
}

impl SourceEncoding {
    const fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "utf-8",
            Self::Windows1252 => "windows-1252",
            Self::Latin1 => "latin1",
        }
    }
}

fn resolve_source_encoding(directives: &SliceDirectives) -> Result<SourceEncoding, RefusalPayload> {
    match directives.encoding.as_deref() {
        None => Ok(SourceEncoding::Utf8),
        Some(label) if label.eq_ignore_ascii_case("utf-8") => Ok(SourceEncoding::Utf8),
        Some(label) if label.eq_ignore_ascii_case("utf8") => Ok(SourceEncoding::Utf8),
        Some(label) if label.eq_ignore_ascii_case("windows-1252") => {
            Ok(SourceEncoding::Windows1252)
        }
        Some(label) if label.eq_ignore_ascii_case("cp1252") => Ok(SourceEncoding::Windows1252),
        Some(label) if label.eq_ignore_ascii_case("latin1") => Ok(SourceEncoding::Latin1),
        Some(label) if label.eq_ignore_ascii_case("latin-1") => Ok(SourceEncoding::Latin1),
        Some(label) if label.eq_ignore_ascii_case("iso-8859-1") => Ok(SourceEncoding::Latin1),
        Some(label) => Err(RefusalPayload::invalid_schema_single(
            "pre_parse.slice.encoding",
            format!("unsupported encoding '{label}'"),
        )),
    }
}

fn read_source_bytes(path: &Path) -> Result<Vec<u8>, RefusalPayload> {
    fs::read(path)
        .map_err(|error| RefusalPayload::io(path.display().to_string(), error.to_string()))
}

fn parse_physical_rows(
    path: &Path,
    bytes: &[u8],
    delimiter: u8,
    source_encoding: SourceEncoding,
) -> Result<Vec<Vec<String>>, RefusalPayload> {
    let decoded = decode_source_bytes(path, bytes, source_encoding)?;
    let bytes = preserve_blank_lines_as_csv_records(decoded.as_bytes());
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(bytes.as_slice());
    let mut rows = Vec::new();

    for record in reader.records() {
        let record = record.map_err(|error| {
            RefusalPayload::csv_parse(path.display().to_string(), error.to_string())
        })?;
        if record.len() == 1 && record.get(0) == Some(BLANK_ROW_SENTINEL) {
            rows.push(Vec::new());
        } else {
            rows.push(record.iter().map(ToOwned::to_owned).collect());
        }
    }

    if rows.is_empty() {
        return Err(RefusalPayload::empty_with_reason(
            path.display().to_string(),
            "no rows",
        ));
    }
    Ok(rows)
}

fn decode_source_bytes(
    path: &Path,
    bytes: &[u8],
    source_encoding: SourceEncoding,
) -> Result<String, RefusalPayload> {
    match source_encoding {
        SourceEncoding::Utf8 => {
            std::str::from_utf8(bytes)
                .map(ToOwned::to_owned)
                .map_err(|error| {
                    invalid_source_encoding(path, bytes, source_encoding, error.valid_up_to())
                })
        }
        SourceEncoding::Windows1252 => {
            decode_single_byte_encoding(path, bytes, source_encoding, encoding_rs::WINDOWS_1252)
        }
        SourceEncoding::Latin1 => decode_single_byte_encoding(
            path,
            bytes,
            source_encoding,
            encoding_rs::Encoding::for_label(b"latin1").expect("latin1 encoding label is built in"),
        ),
    }
}

fn decode_single_byte_encoding(
    path: &Path,
    bytes: &[u8],
    source_encoding: SourceEncoding,
    encoding: &'static encoding_rs::Encoding,
) -> Result<String, RefusalPayload> {
    let (decoded, had_errors) = encoding.decode_without_bom_handling(bytes);
    if had_errors {
        return Err(invalid_source_encoding(path, bytes, source_encoding, 0));
    }
    Ok(decoded.into_owned())
}

fn invalid_source_encoding(
    path: &Path,
    bytes: &[u8],
    source_encoding: SourceEncoding,
    byte_offset: usize,
) -> RefusalPayload {
    let physical_row = physical_row_at_byte_offset(bytes, byte_offset);
    RefusalPayload::csv_parse(
        path.display().to_string(),
        format!(
            "invalid {} source bytes at physical row {physical_row}, byte offset {byte_offset}",
            source_encoding.label()
        ),
    )
}

fn physical_row_at_byte_offset(bytes: &[u8], byte_offset: usize) -> usize {
    let mut row = 1usize;
    let mut index = 0usize;
    let limit = byte_offset.min(bytes.len());

    while index < limit {
        match bytes[index] {
            b'\n' => {
                row += 1;
                index += 1;
            }
            b'\r' => {
                row += 1;
                if bytes.get(index + 1) == Some(&b'\n') {
                    index += 2;
                } else {
                    index += 1;
                }
            }
            _ => {
                index += 1;
            }
        }
    }

    row
}

const BLANK_ROW_SENTINEL: &str = "\u{1e}profile_blank_row\u{1e}";

fn preserve_blank_lines_as_csv_records(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(bytes.len());
    let mut in_quotes = false;
    let mut line_start = 0usize;
    let mut index = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                if in_quotes && bytes.get(index + 1) == Some(&b'"') {
                    index += 2;
                } else {
                    in_quotes = !in_quotes;
                    index += 1;
                }
            }
            b'\n' if !in_quotes => {
                let line_end = if index > line_start && bytes[index - 1] == b'\r' {
                    index - 1
                } else {
                    index
                };
                append_physical_line(bytes, line_start, line_end, index + 1, &mut output);
                index += 1;
                line_start = index;
            }
            b'\r' if !in_quotes => {
                if bytes.get(index + 1) == Some(&b'\n') {
                    append_physical_line(bytes, line_start, index, index + 2, &mut output);
                    index += 2;
                    line_start = index;
                    continue;
                }
                append_physical_line(bytes, line_start, index, index + 1, &mut output);
                index += 1;
                line_start = index;
            }
            _ => {
                index += 1;
            }
        }
    }

    if line_start < bytes.len() {
        append_physical_line(bytes, line_start, bytes.len(), bytes.len(), &mut output);
    }

    output
}

fn append_physical_line(
    bytes: &[u8],
    line_start: usize,
    line_end: usize,
    terminator_end: usize,
    output: &mut Vec<u8>,
) {
    if is_blank_physical_line(&bytes[line_start..line_end]) {
        output.extend_from_slice(BLANK_ROW_SENTINEL.as_bytes());
        output.extend_from_slice(&bytes[line_end..terminator_end]);
    } else {
        output.extend_from_slice(&bytes[line_start..terminator_end]);
    }
}

fn is_blank_physical_line(bytes: &[u8]) -> bool {
    bytes.iter().all(u8::is_ascii_whitespace)
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

#[derive(Debug, Clone)]
struct CanonicalHeaderMaterialization {
    headers: Vec<String>,
    column_registry_hash: String,
    mapping: Vec<HeaderMapping>,
    unmapped: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct HeaderMapping {
    position: usize,
    raw: String,
    canonical: String,
    mapped: bool,
}

fn canonicalize_slice_headers(
    profile_path: Option<&Path>,
    profile: Option<&Profile>,
    headers: &[String],
) -> Result<Option<CanonicalHeaderMaterialization>, RefusalPayload> {
    let Some(profile) = profile else {
        return Ok(None);
    };
    let Some(registry_ref) = profile.column_registry.as_deref() else {
        return Ok(None);
    };
    let profile_path = profile_path.ok_or_else(|| {
        RefusalPayload::invalid_schema_single(
            "profile",
            "column registry canonicalization requires a resolved profile path",
        )
    })?;
    let registry_dir = resolve_registry_path(profile_path, registry_ref);
    let aliases = load_column_registry_aliases(&registry_dir)?;
    let column_registry_hash = profile
        .column_registry_hash
        .clone()
        .map(Ok)
        .unwrap_or_else(|| registry_content_hash(&registry_dir))?;
    let mut canonical_to_raw: HashMap<String, String> = HashMap::new();
    let mut unmapped_seen = HashSet::new();
    let mut output_headers = Vec::with_capacity(headers.len());
    let mut mapping = Vec::with_capacity(headers.len());
    let mut unmapped = Vec::new();

    for (index, raw) in headers.iter().enumerate() {
        let canonical = aliases.get(raw).cloned().unwrap_or_else(|| raw.to_owned());
        if let Some(first_raw) = canonical_to_raw.get(&canonical) {
            if first_raw != raw {
                return Err(canonical_header_collision(&canonical, first_raw, raw));
            }
        } else {
            canonical_to_raw.insert(canonical.clone(), raw.clone());
        }

        let mapped = aliases.contains_key(raw);
        if !mapped && unmapped_seen.insert(raw.clone()) {
            unmapped.push(raw.clone());
        }
        output_headers.push(canonical.clone());
        mapping.push(HeaderMapping {
            position: index + 1,
            raw: raw.clone(),
            canonical,
            mapped,
        });
    }

    Ok(Some(CanonicalHeaderMaterialization {
        headers: output_headers,
        column_registry_hash,
        mapping,
        unmapped,
    }))
}

fn canonical_header_collision(
    canonical: &str,
    first_raw: &str,
    second_raw: &str,
) -> RefusalPayload {
    RefusalPayload::invalid_schema_single(
        "column_registry",
        format!(
            "ambiguous canonical header '{canonical}': physical headers '{first_raw}' and '{second_raw}' both resolve to it"
        ),
    )
}

fn canonical_header_summary(canonicalization: Option<&CanonicalHeaderMaterialization>) -> Value {
    match canonicalization {
        Some(canonicalization) => json!({
            "applied": true,
            "canonicalizer_version": CANONICALIZER_VERSION,
            "mapped_count": canonicalization
                .mapping
                .iter()
                .filter(|entry| entry.mapped)
                .count(),
            "unmapped_count": canonicalization.unmapped.len()
        }),
        None => json!({
            "applied": false
        }),
    }
}

struct SliceManifestInputs<'a> {
    args: &'a SliceArgs,
    profile: Option<&'a Profile>,
    directives: &'a SliceDirectives,
    plan: &'a SlicePlan,
    slice: &'a SliceOutput,
    rows: &'a [Vec<String>],
    input_hash: &'a str,
    output_hash: &'a str,
    source_encoding: SourceEncoding,
    canonicalization: Option<&'a CanonicalHeaderMaterialization>,
}

fn build_manifest(inputs: SliceManifestInputs<'_>) -> Value {
    let SliceManifestInputs {
        args,
        profile,
        directives,
        plan,
        slice,
        rows,
        input_hash,
        output_hash,
        source_encoding,
        canonicalization,
    } = inputs;

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
        "profile_sha256": profile.and_then(|profile| profile.profile_sha256.clone()),
        "fingerprint_ref": profile.and_then(|profile| profile.fingerprint_ref.clone()),
        "input_hash": input_hash,
        "column_registry_hash": canonicalization
            .map(|canonicalization| canonicalization.column_registry_hash.clone()),
        "canonicalizer_version": canonicalization.map(|_| CANONICALIZER_VERSION),
        "mapping": canonicalization
            .map(|canonicalization| json!(&canonicalization.mapping))
            .unwrap_or_else(|| json!([])),
        "unmapped": canonicalization
            .map(|canonicalization| json!(&canonicalization.unmapped))
            .unwrap_or_else(|| json!([])),
        "source_encoding": source_encoding.label(),
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
