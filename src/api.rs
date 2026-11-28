//! Filesystem-free profile operations.
//!
//! These entrypoints operate only on caller-owned bytes and explicit values. They do not read
//! environment variables, the clock, profiles, registries, or the witness ledger. The CLI keeps
//! its existing filesystem and witness shell and can use these same result shapes when embedding
//! profile in another process.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Cursor;

use serde::Serialize;
use serde_json::{Value, json};

use crate::redaction::{Kind, Marker, RedactionKey};
use crate::refusal::RefusalPayload;
use crate::schema::{
    ColumnMetadata, ColumnType, Equivalence, Profile, ProfileFormat, ProfileStatus, ValidationMode,
    canonicalize_profile_column, validate_profile,
};

/// Explicit inputs for the in-memory APIs. The default is CSV, ordinal redaction, and no sheet.
#[derive(Clone, Debug, Default)]
pub struct BytesOptions {
    /// Optional `.csv`/`.tsv`/tabular name used for format and delimiter selection.
    pub name_hint: Option<String>,
    /// Optional worksheet name or zero-based index, used when the tabular feature is enabled.
    pub sheet: Option<String>,
    /// Explicitly authorize dataset names and numeric examples in result values.
    pub explicit: bool,
    /// An already validated keyed-redaction key. The API never reads PROFILE_REDACTION_KEY.
    pub redaction_key: Option<RedactionKey>,
    /// Override the CSV delimiter. `None` uses comma, or tab for a `.tsv` name hint.
    pub delimiter: Option<u8>,
}

/// The result of [`slice_bytes`]. `csv` is the canonical sliced artifact; `result` is the same
/// redacted metadata shape used by the CLI's JSON result, without filesystem-only path fields.
#[derive(Clone, Debug, PartialEq)]
pub struct SliceBytesOutput {
    pub csv: Vec<u8>,
    pub result: Value,
}

#[derive(Debug, Clone)]
struct Dataset {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

fn input_label(options: &BytesOptions) -> String {
    options
        .name_hint
        .clone()
        .unwrap_or_else(|| "<bytes>".to_owned())
}

fn is_tsv(options: &BytesOptions) -> bool {
    options.name_hint.as_deref().is_some_and(|name| {
        name.rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("tsv"))
    })
}

fn delimiter(options: &BytesOptions) -> u8 {
    options
        .delimiter
        .unwrap_or(if is_tsv(options) { b'\t' } else { b',' })
}

fn marker_for(options: &BytesOptions) -> Marker {
    if options.explicit {
        Marker::new(None)
    } else {
        Marker::new(options.redaction_key.clone())
    }
}

fn parse_csv(bytes: &[u8], options: &BytesOptions) -> Result<Dataset, RefusalPayload> {
    let label = input_label(options);
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter(options))
        .from_reader(Cursor::new(bytes));
    let headers = reader
        .headers()
        .map_err(|error| RefusalPayload::csv_parse(label.clone(), error.to_string()))?
        .iter()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if headers.is_empty() {
        return Err(RefusalPayload::empty_with_reason(label, "no header row"));
    }
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record
            .map_err(|error| RefusalPayload::csv_parse(input_label(options), error.to_string()))?;
        rows.push(
            (0..headers.len())
                .map(|index| record.get(index).unwrap_or_default().to_owned())
                .collect(),
        );
    }
    if rows.is_empty() {
        return Err(RefusalPayload::empty_with_reason(
            input_label(options),
            "no data rows",
        ));
    }
    Ok(Dataset { headers, rows })
}

#[cfg(feature = "tabular")]
fn parse_tabular(bytes: &[u8], options: &BytesOptions) -> Result<Dataset, RefusalPayload> {
    use cmdrvl_tabular::{OpenOptions, SheetSel, open_bytes};

    let label = input_label(options);
    let tabular_options = OpenOptions {
        sheet: options.sheet.as_deref().map(SheetSel::parse),
        ..OpenOptions::default()
    };
    let mut table = open_bytes(
        bytes.to_vec(),
        options.name_hint.as_deref(),
        &tabular_options,
    )
    .map_err(|refusal| {
        let context = serde_json::to_value(&refusal.context)
            .unwrap_or_else(|_| Value::Object(Default::default()));
        RefusalPayload::input_format(
            label.clone(),
            refusal.code.as_str(),
            refusal.detail,
            context,
        )
    })?;
    let headers = table
        .headers()
        .iter()
        .map(|header| String::from_utf8_lossy(header).into_owned())
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for row in table.rows() {
        let row = row.map_err(|refusal| {
            let context = serde_json::to_value(&refusal.context)
                .unwrap_or_else(|_| Value::Object(Default::default()));
            RefusalPayload::input_format(
                label.clone(),
                refusal.code.as_str(),
                refusal.detail,
                context,
            )
        })?;
        rows.push(
            (0..headers.len())
                .map(|index| {
                    row.cells
                        .get(index)
                        .map(|cell| String::from_utf8_lossy(cell.as_bytes()).into_owned())
                        .unwrap_or_default()
                })
                .collect(),
        );
    }
    Ok(Dataset { headers, rows })
}

fn parse_dataset(bytes: &[u8], options: &BytesOptions) -> Result<Dataset, RefusalPayload> {
    let tabular_name = options.name_hint.as_deref().is_some_and(|name| {
        name.rsplit_once('.').is_some_and(|(_, extension)| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jsonl" | "ndjson" | "parquet" | "xlsx" | "xlsm" | "xls"
            )
        })
    });
    #[cfg(feature = "tabular")]
    if tabular_name {
        return parse_tabular(bytes, options);
    }
    let _ = tabular_name;
    parse_csv(bytes, options)
}

#[derive(Debug, Default)]
struct InspectAccumulator {
    null_count: usize,
    values: HashSet<String>,
    example: Option<String>,
    observed_types: [bool; 4],
    numeric_values: Vec<f64>,
    saw_non_numeric: bool,
}

impl InspectAccumulator {
    fn observe(&mut self, value: &str) {
        if value.trim().is_empty() {
            self.null_count += 1;
            return;
        }
        if self.example.is_none() {
            self.example = Some(value.to_owned());
        }
        self.values.insert(value.to_owned());
        if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
            self.observed_types[2] = true;
            self.saw_non_numeric = true;
        } else if value.parse::<i64>().is_ok() {
            self.observed_types[0] = true;
            if let Ok(number) = value.parse::<f64>() {
                self.numeric_values.push(number);
            }
        } else if let Ok(number) = value.parse::<f64>()
            && number.is_finite()
        {
            self.observed_types[1] = true;
            self.numeric_values.push(number);
        } else {
            self.observed_types[3] = true;
            self.saw_non_numeric = true;
        }
    }

    fn inferred_type(&self) -> ColumnType {
        match self.observed_types {
            [true, false, false, false] => ColumnType::Integer,
            [true, true, false, false] | [false, true, false, false] => ColumnType::Number,
            [false, false, true, false] => ColumnType::Boolean,
            [false, false, false, true] => ColumnType::Text,
            _ => ColumnType::Unknown,
        }
    }

    fn numeric_summary(&self, explicit: bool) -> Value {
        if self.saw_non_numeric || self.numeric_values.is_empty() {
            return Value::Null;
        }
        let mut values = self.numeric_values.clone();
        values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
        let min = values[0];
        let max = values[values.len() - 1];
        let p50 = quantile(&values, 0.50);
        let p95 = quantile(&values, 0.95);
        json!({
            "min": if explicit { json!(min) } else { Value::Null },
            "max": if explicit { json!(max) } else { Value::Null },
            "p50": if explicit { json!(p50) } else { Value::Null },
            "p95": if explicit { json!(p95) } else { Value::Null },
        })
    }
}

fn quantile(values: &[f64], probability: f64) -> f64 {
    let position = (values.len() - 1) as f64 * probability;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        return values[lower];
    }
    let lower_value = values[lower];
    let upper_value = values[upper];
    let fraction = position - lower as f64;
    let difference = upper_value - lower_value;
    let direct = lower_value + difference * fraction;
    if direct.is_finite() && (difference == 0.0 || difference * fraction != 0.0) {
        direct
    } else {
        let scale = lower_value.abs().max(upper_value.abs());
        if scale == 0.0 {
            0.0
        } else {
            ((lower_value / scale) * (1.0 - fraction) + (upper_value / scale) * fraction) * scale
        }
    }
}

/// Inspect a CSV byte slice with the same redacted result fields as `profile inspect`.
pub fn inspect_bytes(bytes: &[u8], explicit: bool) -> Result<Value, RefusalPayload> {
    inspect_bytes_with_options(
        bytes,
        &BytesOptions {
            explicit,
            ..Default::default()
        },
    )
}

/// Inspect bytes with an explicit format/name and optional keyed redaction configuration.
pub fn inspect_bytes_with_options(
    bytes: &[u8],
    options: &BytesOptions,
) -> Result<Value, RefusalPayload> {
    let dataset = parse_dataset(bytes, options)?;
    let mut accumulators = (0..dataset.headers.len())
        .map(|_| InspectAccumulator::default())
        .collect::<Vec<_>>();
    for row in &dataset.rows {
        for (index, accumulator) in accumulators.iter_mut().enumerate() {
            accumulator.observe(row.get(index).map(String::as_str).unwrap_or_default());
        }
    }

    let mut marker = marker_for(options);
    let columns = dataset
        .headers
        .iter()
        .zip(&accumulators)
        .map(|(name, accumulator)| {
            let null_rate = accumulator.null_count as f64 / dataset.rows.len() as f64;
            let mut column = json!({
                "id": marker.mark(Kind::Column, name),
                "type": accumulator.inferred_type().as_str(),
                "null_count": accumulator.null_count,
                "null_rate": null_rate,
                "distinct_count": accumulator.values.len(),
                "uniqueness": accumulator.values.len() as f64 / dataset.rows.len() as f64,
                "numeric": accumulator.numeric_summary(options.explicit),
            });
            if options.explicit {
                column["name"] = json!(name);
                column["example"] = json!(accumulator.example.clone().unwrap_or_default());
            }
            column
        })
        .collect::<Vec<_>>();

    let candidates = suggest_dataset(&dataset, 5, 3, options)
        .and_then(|suggestions| {
            suggestions.get("candidates").cloned().ok_or_else(|| {
                RefusalPayload::invalid_schema_single("suggest-key", "missing candidates")
            })
        })
        .unwrap_or_else(|_| json!([]));
    let (format, dialect) = format_metadata(options);
    Ok(json!({
        "format": format,
        "dialect": {
            "name": dialect.0,
            "delimiter": dialect.1,
            "source": "shared cmdrvl-tabular reader or profile CSV reader"
        },
        "row_count": dataset.rows.len(),
        "column_count": dataset.headers.len(),
        "columns": columns,
        "redaction": if options.explicit { "explicit" } else { marker.mode() },
        "key_candidates": candidates,
        "suggestions": {
            "header_row": 1,
            "preamble_rows": 0,
            "confidence": "default",
            "note": "No preamble was inferred; use fingerprint peek for messy CSV row-shape analysis."
        }
    }))
}

fn format_metadata(options: &BytesOptions) -> (&'static str, (&'static str, &'static str)) {
    let extension = options
        .name_hint
        .as_deref()
        .and_then(|name| name.rsplit_once('.').map(|(_, extension)| extension))
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("tsv") => ("tsv", ("tsv", "\\t")),
        Some("jsonl" | "ndjson") => ("jsonl", ("jsonl", "none")),
        Some("parquet") => ("parquet", ("parquet", "none")),
        Some("xlsx" | "xlsm" | "xls") => ("xlsx", ("xlsx", "none")),
        _ => ("csv", ("csv", ",")),
    }
}

#[derive(Debug, Clone, Serialize)]
struct KeyCandidate {
    #[serde(skip)]
    indices: Vec<usize>,
    column: String,
    columns: Vec<String>,
    position: usize,
    arity: usize,
    uniqueness: f64,
    null_rate: f64,
    stability_score: u8,
    viable: bool,
    duplicate_count: usize,
    incomplete_count: usize,
    whitespace_collision_count: usize,
    case_collision_count: usize,
    rank: usize,
}

type Hash128 = [u8; 16];
const AUTO_KEY_UNIQUENESS_THRESHOLD: f64 = 0.95;

#[derive(Debug, Default)]
struct CandidateStats {
    normalized: HashMap<Hash128, u8>,
    lowercase: HashMap<Hash128, u8>,
    incomplete_count: usize,
    duplicate_count: usize,
    whitespace_collision_count: usize,
    case_collision_count: usize,
}

impl CandidateStats {
    fn observe(&mut self, values: &[&str]) {
        let normalized = values
            .iter()
            .map(|value| ascii_trim(value.as_bytes()))
            .collect::<Vec<_>>();
        if normalized.iter().any(|value| value.is_empty()) {
            self.incomplete_count += 1;
            return;
        }
        let padded = values
            .iter()
            .zip(&normalized)
            .any(|(value, normalized)| value.as_bytes() != *normalized);
        let key = hash_components(&normalized);
        let state = self.normalized.entry(key).or_default();
        if *state & 1 != 0 {
            self.duplicate_count += 1;
        }
        if padded && *state & 2 != 0 {
            self.whitespace_collision_count += 1;
        }
        if !padded && *state & 4 != 0 {
            self.whitespace_collision_count += 1;
        }
        *state |= 1;
        *state |= if padded { 4 } else { 2 };

        let lowercase = normalized
            .iter()
            .map(|value| value.to_ascii_lowercase())
            .collect::<Vec<_>>();
        let lower_key = hash_components(&lowercase.iter().map(Vec::as_slice).collect::<Vec<_>>());
        let all_lower = normalized
            .iter()
            .zip(&lowercase)
            .all(|(value, lowercase)| *value == lowercase.as_slice());
        let lower_state = self.lowercase.entry(lower_key).or_default();
        if all_lower && *lower_state & 2 != 0 {
            self.case_collision_count += 1;
        }
        if !all_lower && *lower_state & 1 != 0 {
            self.case_collision_count += 1;
        }
        *lower_state |= if all_lower { 1 } else { 2 };
    }

    fn unique_count(&self) -> usize {
        self.normalized.len()
    }
}

fn suggest_dataset(
    dataset: &Dataset,
    top: usize,
    max_arity: usize,
    options: &BytesOptions,
) -> Result<Value, RefusalPayload> {
    let specs = candidate_specs(dataset.headers.len(), max_arity);
    let mut stats = specs
        .iter()
        .map(|_| CandidateStats::default())
        .collect::<Vec<_>>();
    for row in &dataset.rows {
        for (spec, candidate_stats) in specs.iter().zip(&mut stats) {
            let values = spec
                .iter()
                .map(|index| row.get(*index).map(String::as_str).unwrap_or_default())
                .collect::<Vec<_>>();
            candidate_stats.observe(&values);
        }
    }
    if dataset.rows.is_empty() {
        return Err(RefusalPayload::empty_with_reason(
            input_label(options),
            "no data rows",
        ));
    }
    let singles = specs.iter().zip(&stats).filter(|(spec, _)| spec.len() == 1);
    let has_perfect_single = singles
        .clone()
        .any(|(_, stat)| stat.incomplete_count == 0 && stat.unique_count() == dataset.rows.len());
    let include_composites = max_arity > 1 || !has_perfect_single;
    let mut candidates = specs
        .iter()
        .zip(stats)
        .filter(|(spec, _)| spec.len() == 1 || include_composites)
        .map(|(spec, stat)| candidate_from_stats(spec, stat, &dataset.headers, dataset.rows.len()))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .uniqueness
            .partial_cmp(&left.uniqueness)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                left.null_rate
                    .partial_cmp(&right.null_rate)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(right.stability_score.cmp(&left.stability_score))
            .then(left.arity.cmp(&right.arity))
            .then(left.position.cmp(&right.position))
            .then(left.indices.cmp(&right.indices))
    });
    for (index, candidate) in candidates.iter_mut().enumerate() {
        candidate.rank = index + 1;
    }
    let mut candidates = candidates.into_iter().take(top).collect::<Vec<_>>();
    if !options.explicit {
        let mut marker = marker_for(options);
        let refs = dataset
            .headers
            .iter()
            .map(|name| marker.mark(Kind::Column, name))
            .collect::<Vec<_>>();
        for candidate in &mut candidates {
            candidate.columns = candidate
                .indices
                .iter()
                .map(|index| refs[*index].clone())
                .collect();
            candidate.column = candidate.columns.join(" + ");
        }
    }
    Ok(json!({
        "candidates": candidates,
        "redaction": if options.explicit { "explicit" } else { marker_for(options).mode() },
        "top": top,
        "max_arity": max_arity,
        "ranking": {
            "order": ["uniqueness_desc", "null_rate_asc", "stability_desc", "arity_asc", "position_asc"],
            "stability_signals": ["*_id", "*_key", "*_number"],
            "composite_semantics": "ordered tuples; ASCII-trim each component; literal NA/NULL remain values"
        }
    }))
}

fn candidate_specs(column_count: usize, max_arity: usize) -> Vec<Vec<usize>> {
    let mut specs = (0..column_count)
        .map(|index| vec![index])
        .collect::<Vec<_>>();
    if max_arity >= 2 {
        for left in 0..column_count {
            for right in left + 1..column_count {
                specs.push(vec![left, right]);
            }
        }
    }
    if max_arity >= 3 {
        for first in 0..column_count {
            for second in first + 1..column_count {
                for third in second + 1..column_count {
                    specs.push(vec![first, second, third]);
                }
            }
        }
    }
    specs
}

fn candidate_from_stats(
    spec: &[usize],
    stats: CandidateStats,
    headers: &[String],
    rows: usize,
) -> KeyCandidate {
    let columns = spec
        .iter()
        .map(|index| headers[*index].clone())
        .collect::<Vec<_>>();
    let uniqueness = stats.unique_count() as f64 / rows as f64;
    let null_rate = stats.incomplete_count as f64 / rows as f64;
    let stability_score = columns
        .iter()
        .map(|column| stability_score(column))
        .sum::<u8>()
        .min(3);
    KeyCandidate {
        indices: spec.to_vec(),
        column: columns.join(" + "),
        columns,
        position: spec[0],
        arity: spec.len(),
        uniqueness,
        null_rate,
        stability_score,
        viable: uniqueness >= AUTO_KEY_UNIQUENESS_THRESHOLD && stats.incomplete_count == 0,
        duplicate_count: stats.duplicate_count,
        incomplete_count: stats.incomplete_count,
        whitespace_collision_count: stats.whitespace_collision_count,
        case_collision_count: stats.case_collision_count,
        rank: 0,
    }
}

fn hash_components(components: &[&[u8]]) -> Hash128 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"profile.composite-key.v1\0");
    for component in components {
        hasher.update(&(component.len() as u64).to_be_bytes());
        hasher.update(component);
    }
    let mut hash = [0_u8; 16];
    hash.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
    hash
}

fn ascii_trim(value: &[u8]) -> &[u8] {
    let start = value
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t'))
        .unwrap_or(value.len());
    let end = value
        .iter()
        .rposition(|byte| !matches!(byte, b' ' | b'\t'))
        .map_or(start, |index| index + 1);
    &value[start..end]
}

fn stability_score(column_name: &str) -> u8 {
    let normalized = column_name.to_ascii_lowercase();
    if normalized.ends_with("_id") {
        3
    } else if normalized.ends_with("_key") {
        2
    } else if normalized.ends_with("_number") {
        1
    } else {
        0
    }
}

/// Suggest candidate keys from bytes without witness or filesystem access.
pub fn suggest_key_bytes(
    bytes: &[u8],
    top: usize,
    max_arity: usize,
    explicit: bool,
) -> Result<Value, RefusalPayload> {
    suggest_key_bytes_with_options(
        bytes,
        top,
        max_arity,
        &BytesOptions {
            explicit,
            ..Default::default()
        },
    )
}

/// Option-aware form of [`suggest_key_bytes`].
pub fn suggest_key_bytes_with_options(
    bytes: &[u8],
    top: usize,
    max_arity: usize,
    options: &BytesOptions,
) -> Result<Value, RefusalPayload> {
    let dataset = parse_dataset(bytes, options)?;
    suggest_dataset(&dataset, top, max_arity, options)
}

/// Build the same draft domain object as `profile draft init`, using a CSV/TSV byte slice.
pub fn draft_bytes(bytes: &[u8]) -> Result<Profile, RefusalPayload> {
    draft_bytes_with_options(bytes, &BytesOptions::default())
}

/// Option-aware form of [`draft_bytes`].
pub fn draft_bytes_with_options(
    bytes: &[u8],
    options: &BytesOptions,
) -> Result<Profile, RefusalPayload> {
    let dataset = parse_dataset(bytes, options)?;
    let mut accumulators = (0..dataset.headers.len())
        .map(|_| InspectAccumulator::default())
        .collect::<Vec<_>>();
    for row in &dataset.rows {
        for (index, accumulator) in accumulators.iter_mut().enumerate() {
            accumulator.observe(row.get(index).map(String::as_str).unwrap_or_default());
        }
    }
    let columns = dataset
        .headers
        .iter()
        .zip(accumulators)
        .map(|(name, accumulator)| {
            (
                canonicalize_profile_column(name, None),
                ColumnMetadata {
                    type_: Some(accumulator.inferred_type()),
                    nullable: Some(accumulator.null_count > 0),
                    unit: None,
                    sensitivity: None,
                    airlock_class: None,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let profile = Profile {
        schema_version: 1,
        profile_id: None,
        profile_version: None,
        profile_family: None,
        profile_sha256: None,
        frozen: None,
        status: ProfileStatus::Draft,
        format: if is_tsv(options) {
            ProfileFormat::Tsv
        } else {
            ProfileFormat::Csv
        },
        column_registry: None,
        column_registry_hash: None,
        fingerprint_ref: None,
        pre_parse: None,
        columns,
        hashing: None,
        equivalence: Some(Equivalence {
            order: None,
            float_decimals: Some(6),
            trim_strings: Some(true),
        }),
        key: Vec::new(),
        include_columns: dataset.headers,
    };
    validate_profile(&profile, ValidationMode::Validate)?;
    Ok(profile)
}

/// Serialize [`draft_bytes`] using the profile's stable YAML serializer.
pub fn draft_bytes_yaml(bytes: &[u8]) -> Result<String, RefusalPayload> {
    let profile = draft_bytes(bytes)?;
    profile
        .to_yaml()
        .map_err(|error| RefusalPayload::invalid_schema_single("draft", error.to_string()))
}

/// Slice a CSV byte slice after `preamble_skip` physical CSV records. The output is canonical CSV
/// and the metadata uses the CLI's default redaction contract.
pub fn slice_bytes(
    bytes: &[u8],
    preamble_skip: usize,
    explicit: bool,
) -> Result<SliceBytesOutput, RefusalPayload> {
    slice_bytes_with_options(
        bytes,
        preamble_skip,
        &BytesOptions {
            explicit,
            ..Default::default()
        },
    )
}

/// Option-aware form of [`slice_bytes`].
pub fn slice_bytes_with_options(
    bytes: &[u8],
    preamble_skip: usize,
    options: &BytesOptions,
) -> Result<SliceBytesOutput, RefusalPayload> {
    let label = input_label(options);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter(options))
        .from_reader(Cursor::new(bytes));
    let mut rows = Vec::new();
    for record in reader.records() {
        let record =
            record.map_err(|error| RefusalPayload::csv_parse(label.clone(), error.to_string()))?;
        rows.push(record.iter().map(ToOwned::to_owned).collect::<Vec<_>>());
    }
    let header_index = preamble_skip;
    let headers = rows
        .get(header_index)
        .cloned()
        .ok_or_else(|| RefusalPayload::empty_with_reason(label.clone(), "no header row"))?;
    if headers.is_empty() {
        return Err(RefusalPayload::empty_with_reason(label, "no header row"));
    }
    let data_rows = rows
        .iter()
        .skip(header_index + 1)
        .cloned()
        .collect::<Vec<_>>();
    if data_rows.is_empty() {
        return Err(RefusalPayload::empty_with_reason(
            input_label(options),
            "no data rows",
        ));
    }
    let mut writer = csv::WriterBuilder::new()
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());
    writer
        .write_record(&headers)
        .map_err(|error| RefusalPayload::csv_parse(input_label(options), error.to_string()))?;
    for row in &data_rows {
        writer
            .write_record(row)
            .map_err(|error| RefusalPayload::csv_parse(input_label(options), error.to_string()))?;
    }
    let csv = writer
        .into_inner()
        .map_err(|error| RefusalPayload::io(input_label(options), error.to_string()))?;
    let mut marker = marker_for(options);
    let columns = if options.explicit {
        headers.clone()
    } else {
        headers
            .iter()
            .map(|header| marker.mark(Kind::Column, header))
            .collect()
    };
    let mut result = json!({
        "source_encoding": "utf-8",
        "mode": "preamble_skip",
        "directives": { "skip_rows": preamble_skip },
        "rows": { "input_physical_rows": rows.len(), "header_rows": [header_index], "unit_rows": [], "data_starts_at": header_index + 1, "output_data_rows": data_rows.len() },
        "columns": columns,
        "redaction": if options.explicit { "explicit" } else { marker.mode() },
        "output_hash": format!("blake3:{}", blake3::hash(&csv).to_hex()),
    });
    if options.explicit {
        result["slice_csv"] = Value::String(String::from_utf8_lossy(&csv).into_owned());
    }
    Ok(SliceBytesOutput { csv, result })
}

/// Evolve a frozen profile against new headers from bytes, without reading or writing files.
pub fn evolve_profiles(profile: &Profile, bytes: &[u8]) -> Result<Profile, RefusalPayload> {
    let dataset = parse_dataset(bytes, &BytesOptions::default())?;
    validate_profile(profile, ValidationMode::Validate)?;
    if !profile.is_frozen() {
        return Err(RefusalPayload::invalid_schema_single(
            "status",
            "profile evolve requires a frozen input profile",
        ));
    }
    let old_set = profile.include_columns.iter().collect::<HashSet<_>>();
    let new_headers = dataset.headers;
    let new_set = new_headers.iter().collect::<HashSet<_>>();
    let dropped = profile
        .include_columns
        .iter()
        .filter(|column| !new_set.contains(column))
        .cloned()
        .collect::<Vec<_>>();
    let added = new_headers
        .iter()
        .filter(|column| !old_set.contains(column))
        .cloned()
        .collect::<Vec<_>>();
    let mut replacements = HashMap::new();
    for old in &dropped {
        let mut matches = new_headers
            .iter()
            .map(|new| (new, similarity(old, new)))
            .filter(|(_, score)| *score >= 0.8)
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| {
            right
                .1
                .partial_cmp(&left.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if let Some((new, score)) = matches.first()
            && (matches.get(1).is_none() || (score - matches[1].1).abs() >= 0.1)
        {
            replacements.insert(old.clone(), (*new).clone());
        }
    }
    let mut evolved = profile.clone();
    evolved.status = ProfileStatus::Draft;
    evolved.profile_id = None;
    evolved.profile_version = None;
    evolved.profile_family = None;
    evolved.profile_sha256 = None;
    evolved.frozen = None;
    evolved.column_registry_hash = None;
    evolved.include_columns = profile
        .include_columns
        .iter()
        .filter_map(|column| {
            if dropped.contains(column) {
                replacements.get(column).cloned()
            } else {
                Some(column.clone())
            }
        })
        .collect();
    for column in added {
        if !evolved.include_columns.contains(&column) {
            evolved.include_columns.push(column);
        }
    }
    evolved.key = profile
        .key
        .iter()
        .map(|column| {
            replacements
                .get(column)
                .cloned()
                .unwrap_or_else(|| column.clone())
        })
        .collect();
    validate_profile(&evolved, ValidationMode::Validate)?;
    Ok(evolved)
}

/// Alias for [`evolve_profiles`] matching the CLI operation name.
pub fn evolve(profile: &Profile, bytes: &[u8]) -> Result<Profile, RefusalPayload> {
    evolve_profiles(profile, bytes)
}

fn similarity(left: &str, right: &str) -> f64 {
    let left = normalize_header(left);
    let right = normalize_header(right);
    if left == right {
        return 1.0;
    }
    let max_len = left.chars().count().max(right.chars().count());
    if max_len == 0 {
        return 1.0;
    }
    1.0 - (edit_distance(&left, &right) as f64 / max_len as f64)
}

fn normalize_header(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right = right.chars().collect::<Vec<_>>();
    let mut row = (0..=right.len()).collect::<Vec<_>>();
    for (left_index, left_char) in left.chars().enumerate() {
        let mut next = vec![left_index + 1; right.len() + 1];
        for (right_index, right_char) in right.iter().enumerate() {
            next[right_index + 1] = (row[right_index + 1] + 1)
                .min(next[right_index] + 1)
                .min(row[right_index] + usize::from(left_char != *right_char));
        }
        row = next;
    }
    row[right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &[u8] = b"loan_id,balance\nL-1,10\nL-2,20\n";

    #[test]
    fn bytes_apis_are_deterministic_and_do_not_need_filesystem() {
        let first = inspect_bytes(CSV, false).expect("inspect bytes");
        assert_eq!(first, inspect_bytes(CSV, false).expect("repeat inspect"));
        assert_eq!(first["columns"][0]["id"], "redacted:ref:c1");
        assert_eq!(first["columns"][0].get("name"), None);
        let draft = draft_bytes(CSV).expect("draft bytes");
        assert_eq!(draft.include_columns, vec!["loan_id", "balance"]);
        let suggested = suggest_key_bytes(CSV, 5, 3, false).expect("suggest bytes");
        assert_eq!(suggested["candidates"][0]["column"], "redacted:ref:c1");
        let slice =
            slice_bytes(b"banner\nloan_id,balance\nL-1,10\n", 1, false).expect("slice bytes");
        assert_eq!(slice.csv, b"loan_id,balance\nL-1,10\n");
    }

    #[test]
    fn bytes_api_explicit_mode_keeps_names_and_values() {
        let inspected = inspect_bytes(CSV, true).expect("explicit inspect");
        assert_eq!(inspected["columns"][0]["name"], "loan_id");
        assert_eq!(inspected["columns"][1]["numeric"]["min"], 10.0);
        let slice = slice_bytes(CSV, 0, true).expect("explicit slice");
        assert!(
            slice.result["slice_csv"]
                .as_str()
                .unwrap()
                .contains("loan_id")
        );
    }
}
