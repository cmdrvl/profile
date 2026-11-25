//! Drift-guided drafting for a new profile version.
//!
//! `profile evolve` deliberately leaves the frozen input untouched. It compares only
//! headers and profile metadata, writes a mutable draft, and leaves any ambiguous rename
//! for an operator to confirm rather than guessing silently.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::cli::args::{EvolveArgs, EvolveView};
use crate::output::json::{CommandOutput, ProfileRef};
use crate::refusal::RefusalPayload;
use crate::schema::{
    PreParse, Profile, ProfileStatus, ValidationMode, build_header_index,
    canonicalize_header_sequence, load_column_registry_aliases, parse_profile_yaml,
    resolve_registry_path, validate_frozen_registry_hash, validate_profile,
};

pub fn run(args: &EvolveArgs, _no_witness: bool) -> Result<CommandOutput, RefusalPayload> {
    if args.out.exists() {
        return Err(RefusalPayload::io(
            args.out.display().to_string(),
            "target file already exists; choose a new draft path",
        ));
    }

    let original = fs::read_to_string(&args.profile).map_err(|error| {
        RefusalPayload::io(args.profile.display().to_string(), error.to_string())
    })?;
    let old_profile = parse_profile_yaml(&original)?;
    validate_profile(&old_profile, ValidationMode::Validate)?;
    validate_frozen_registry_hash(&args.profile, &old_profile)?;
    if !old_profile.is_frozen() {
        return Err(RefusalPayload::invalid_schema_single(
            "status",
            "profile evolve requires a frozen input profile",
        ));
    }

    let aliases = old_profile
        .column_registry
        .as_deref()
        .map(|registry| {
            load_column_registry_aliases(&resolve_registry_path(&args.profile, registry))
        })
        .transpose()?;

    let peek_pre_parse = args
        .from_peek
        .as_deref()
        .map(load_pre_parse_from_peek)
        .transpose()?;
    let inspection_pre_parse = peek_pre_parse
        .clone()
        .or_else(|| old_profile.pre_parse.clone());
    let inspection_sheet = args.sheet.as_deref().or_else(|| {
        old_profile
            .pre_parse
            .as_ref()
            .and_then(|pre_parse| pre_parse.sheet.as_deref())
    });
    let headers = read_headers(
        &args.against,
        inspection_pre_parse.as_ref(),
        inspection_sheet,
    )?;
    if headers.is_empty() {
        return Err(RefusalPayload::empty_with_reason(
            args.against.display().to_string(),
            "no header row",
        ));
    }

    let raw_headers = headers.clone();
    let header_record = csv::StringRecord::from(headers);
    let canonical_headers = canonicalize_header_sequence(&header_record, aliases.as_ref());
    let available = build_header_index(&header_record, aliases.as_ref());
    let old_include = old_profile.include_columns.clone();
    let old_include_set = old_include.iter().cloned().collect::<HashSet<_>>();
    let new_header_set = canonical_headers.iter().cloned().collect::<HashSet<_>>();

    let added_columns = canonical_headers
        .iter()
        .filter(|column| !old_include_set.contains(*column))
        .cloned()
        .collect::<Vec<_>>();
    let dropped_columns = old_include
        .iter()
        .filter(|column| !new_header_set.contains(*column))
        .cloned()
        .collect::<Vec<_>>();

    let renamed_candidates = rename_candidates(
        &old_include,
        &dropped_columns,
        &raw_headers,
        &canonical_headers,
        aliases.as_ref(),
    );
    let replacements = unambiguous_replacements(&renamed_candidates);

    let mut draft = old_profile.clone();
    draft.status = ProfileStatus::Draft;
    draft.profile_id = None;
    draft.profile_version = None;
    draft.profile_family = None;
    draft.profile_sha256 = None;
    draft.frozen = None;
    draft.column_registry_hash = None;
    draft.include_columns = evolve_columns(
        &old_include,
        &dropped_columns,
        &added_columns,
        &replacements,
    );
    draft.key = old_profile
        .key
        .iter()
        .map(|column| {
            replacements
                .get(column)
                .cloned()
                .unwrap_or_else(|| column.clone())
        })
        .collect();
    if let Some(pre_parse) = peek_pre_parse.clone() {
        draft.pre_parse = Some(pre_parse);
    }
    validate_profile(&draft, ValidationMode::Validate)?;

    let draft_yaml = clean_yaml(&draft)?;
    fs::write(&args.out, draft_yaml)
        .map_err(|error| RefusalPayload::io(args.out.display().to_string(), error.to_string()))?;

    let key_status = key_status(&old_profile, &available, &replacements);
    let slice_shift = slice_shift(old_profile.pre_parse.as_ref(), draft.pre_parse.as_ref());
    let next_profile_version = old_profile.profile_version.unwrap_or(0).saturating_add(1);
    let next_actions = vec![
        format!(
            "profile lint {} --against {}",
            args.out.display(),
            args.against.display()
        ),
        format!(
            "profile freeze {} --family {} --version {} --out <FROZEN_OUTPUT.yaml>",
            args.out.display(),
            old_profile.profile_family.as_deref().unwrap_or("<family>"),
            next_profile_version
        ),
    ];

    let mut result = json!({
        "view": view_name(args.view),
        "output_path": args.out.display().to_string(),
        "next_profile_version": next_profile_version,
        "added_columns": added_columns,
        "dropped_columns": dropped_columns,
        "renamed_candidates": renamed_candidates,
        "slice_shift": slice_shift,
        "key_status": key_status,
        "next_actions": next_actions,
    });
    if matches!(args.view, EvolveView::Full) {
        result["old_profile"] = serde_json::to_value(&old_profile)
            .map_err(|error| RefusalPayload::invalid_schema_single("profile", error.to_string()))?;
        result["new_dataset"] = json!(args.against.display().to_string());
        result["draft_profile"] = serde_json::to_value(&draft)
            .map_err(|error| RefusalPayload::invalid_schema_single("profile", error.to_string()))?;
    }

    Ok(CommandOutput::success(result).with_profile_ref(ProfileRef::from_profile(&old_profile)))
}

fn read_headers(
    path: &Path,
    pre_parse: Option<&PreParse>,
    sheet: Option<&str>,
) -> Result<Vec<String>, RefusalPayload> {
    if let Some(pre_parse) = pre_parse {
        return crate::slice::headers_from_pre_parse(path, pre_parse, sheet);
    }

    let mut reader = crate::tabular_input::open_dataset(path, sheet)?;
    reader
        .headers()
        .map(|headers| headers.iter().map(ToOwned::to_owned).collect())
        .map_err(|error| RefusalPayload::csv_parse(path.display().to_string(), error.to_string()))
}

fn clean_yaml(profile: &Profile) -> Result<String, RefusalPayload> {
    let rendered = profile.to_yaml().map_err(|error| {
        RefusalPayload::invalid_schema_single(
            "profile",
            format!("failed to serialize draft: {error}"),
        )
    })?;
    let rendered = rendered.strip_prefix("---\n").unwrap_or(&rendered);
    Ok(format!("{}\n", rendered.trim_end_matches('\n')))
}

fn evolve_columns(
    old_include: &[String],
    dropped: &[String],
    added: &[String],
    replacements: &HashMap<String, String>,
) -> Vec<String> {
    let dropped = dropped.iter().collect::<HashSet<_>>();
    let mut columns = Vec::new();
    for column in old_include {
        if dropped.contains(column) {
            if let Some(replacement) = replacements.get(column) {
                columns.push(replacement.clone());
            }
        } else {
            columns.push(column.clone());
        }
    }
    for column in added {
        if !columns.contains(column) {
            columns.push(column.clone());
        }
    }
    columns
}

fn key_status(
    profile: &Profile,
    available: &crate::schema::HeaderIndex,
    replacements: &HashMap<String, String>,
) -> Value {
    let missing = profile
        .key
        .iter()
        .filter(|column| {
            available.column_index(column).is_none() && !replacements.contains_key(*column)
        })
        .cloned()
        .collect::<Vec<_>>();
    let repaired = profile
        .key
        .iter()
        .filter_map(|column| {
            replacements
                .get(column)
                .map(|replacement| json!({"from": column, "to": replacement}))
        })
        .collect::<Vec<_>>();
    json!({
        "declared": profile.key,
        "missing": missing,
        "repaired": repaired,
        "preserved": missing.is_empty(),
    })
}

fn rename_candidates(
    old_include: &[String],
    dropped: &[String],
    raw_headers: &[String],
    canonical_headers: &[String],
    aliases: Option<&HashMap<String, String>>,
) -> Vec<Value> {
    let old_set = old_include.iter().collect::<HashSet<_>>();
    let mut candidates = Vec::new();

    if let Some(aliases) = aliases {
        for raw in raw_headers {
            if let Some(canonical) = aliases.get(raw)
                && raw != canonical
                && old_set.contains(canonical)
            {
                candidates.push(json!({
                    "old": canonical,
                    "new": raw,
                    "canonical": canonical,
                    "score": 1.0,
                    "source": "column_registry",
                    "ambiguous": false,
                }));
            }
        }
    }

    for old in dropped {
        let mut matches = canonical_headers
            .iter()
            .map(|new| (new, similarity(old, new)))
            .filter(|(_, score)| *score >= 0.5)
            .collect::<Vec<_>>();
        matches.sort_by(|(left_name, left_score), (right_name, right_score)| {
            right_score
                .partial_cmp(left_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left_name.cmp(right_name))
        });
        if let Some((new, score)) = matches.first() {
            let ambiguous = matches
                .get(1)
                .is_some_and(|(_, other_score)| (*score - *other_score).abs() < 0.1);
            candidates.push(json!({
                "old": old,
                "new": new,
                "score": score,
                "source": "header_similarity",
                "ambiguous": ambiguous,
            }));
        }
    }

    candidates
}

fn unambiguous_replacements(candidates: &[Value]) -> HashMap<String, String> {
    candidates
        .iter()
        .filter(|candidate| {
            !candidate
                .get("ambiguous")
                .and_then(Value::as_bool)
                .unwrap_or(true)
        })
        .filter_map(|candidate| {
            let old = candidate.get("old")?.as_str()?;
            let new = candidate.get("new")?.as_str()?;
            let score = candidate.get("score")?.as_f64()?;
            let source = candidate.get("source")?.as_str()?;
            if source == "column_registry" || score >= 0.8 {
                Some((old.to_string(), new.to_string()))
            } else {
                None
            }
        })
        .collect()
}

fn slice_shift(old: Option<&PreParse>, new: Option<&PreParse>) -> Value {
    let old_slice = old.map(|profile| &profile.slice);
    let new_slice = new.map(|profile| &profile.slice);
    let changed = old_slice != new_slice;
    json!({
        "changed": changed,
        "old": old_slice,
        "new": new_slice,
        "reason": if changed { "peek directives differ from frozen profile" } else { "no shift detected" },
    })
}

fn load_pre_parse_from_peek(path: &Path) -> Result<PreParse, RefusalPayload> {
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
                "peek JSON did not include result.suggestions.profile_pre_parse",
            )
        })?;
    let mode = match suggestion.get("mode").and_then(Value::as_str) {
        Some("preamble_skip") => crate::schema::SliceMode::PreambleSkip,
        Some("multi_row_header") => crate::schema::SliceMode::MultiRowHeader,
        Some("preamble_with_units") => crate::schema::SliceMode::PreambleWithUnits,
        Some(other) => {
            return Err(RefusalPayload::invalid_schema_single(
                "from_peek.mode",
                format!("unsupported peek mode '{other}'"),
            ));
        }
        None => return Err(RefusalPayload::missing_field("from_peek.mode")),
    };
    let read_usize = |value: &Value, key: &str| {
        value
            .get(key)
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
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
                    .collect()
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
    Ok(PreParse {
        sheet: None,
        fingerprint_extract: None,
        expected_shape,
        slice,
    })
}

fn view_name(view: EvolveView) -> &'static str {
    match view {
        EvolveView::Brief => "brief",
        EvolveView::Delta => "delta",
        EvolveView::Full => "full",
    }
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
