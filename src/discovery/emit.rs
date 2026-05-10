use std::fs;
use std::path::Path;

use csv::ReaderBuilder;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::cli::args::EmitDiscoveryArgs;
use crate::output::json::CommandOutput;
use crate::refusal::RefusalPayload;

pub fn run(args: &EmitDiscoveryArgs) -> Result<CommandOutput, RefusalPayload> {
    let source_bytes = fs::read(&args.source_file).map_err(|error| {
        RefusalPayload::io(args.source_file.display().to_string(), error.to_string())
    })?;
    let source_text = std::str::from_utf8(&source_bytes).map_err(|_| {
        RefusalPayload::csv_parse(
            args.source_file.display().to_string(),
            "E_NOT_TEXT: source file is not UTF-8 text",
        )
    })?;

    let source_kind = resolve_source_kind(args);
    let candidate_id = format!("{source_kind}.candidate.v0");
    let next_action = format!("fingerprint template promote --as {source_kind}.v1");

    let source_lines = source_text.lines().collect::<Vec<_>>();
    if args.skip_rows > source_lines.len() {
        return Err(RefusalPayload::csv_parse(
            args.sliced_csv.display().to_string(),
            format!(
                "E_BAD_SLICE: skip_rows {} exceeds source line count {}",
                args.skip_rows,
                source_lines.len()
            ),
        ));
    }
    let preamble_lines = source_lines
        .iter()
        .take(args.skip_rows)
        .map(|line| (*line).to_owned())
        .collect::<Vec<_>>();

    let sliced = parse_sliced_csv(&args.sliced_csv)?;
    let signal_strength = header_signal_strength(&sliced.headers);

    let result = json!({
        "version": "profile.discovery.v0",
        "outcome": "DISCOVERED",
        "candidate_template": {
            "id": candidate_id,
            "source_kind": source_kind,
            "skip_rows": args.skip_rows,
            "header_row_offset": args.skip_rows,
            "column_count": sliced.headers.len(),
            "headers": sliced.headers,
            "evidence": {
                "source_file_sha256": sha256_prefixed(&source_bytes),
                "lines_scanned": source_lines.len(),
                "consistent_column_count_below_offset": true,
                "preamble_lines": preamble_lines,
                "header_row_signal_strength": signal_strength
            }
        },
        "next_action": next_action
    });

    Ok(CommandOutput::success(result))
}

#[derive(Debug)]
struct SlicedCsv {
    headers: Vec<String>,
}

fn parse_sliced_csv(path: &Path) -> Result<SlicedCsv, RefusalPayload> {
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_path(path)
        .map_err(|error| {
            RefusalPayload::csv_parse(path.display().to_string(), error.to_string())
        })?;

    let headers_record = reader
        .headers()
        .map_err(|error| RefusalPayload::csv_parse(path.display().to_string(), error.to_string()))?
        .clone();
    if headers_record.is_empty() {
        return Err(RefusalPayload::empty_with_reason(
            path.display().to_string(),
            "no header columns found",
        ));
    }

    let column_count = headers_record.len();
    for (index, row) in reader.records().enumerate() {
        let row = row.map_err(|error| {
            RefusalPayload::csv_parse(path.display().to_string(), error.to_string())
        })?;
        if row.len() != column_count {
            return Err(RefusalPayload::csv_parse(
                path.display().to_string(),
                format!(
                    "E_BAD_SLICE: row {} has {} columns, expected {}",
                    index + 2,
                    row.len(),
                    column_count
                ),
            ));
        }
    }

    Ok(SlicedCsv {
        headers: headers_record.iter().map(ToOwned::to_owned).collect(),
    })
}

fn header_signal_strength(headers: &[String]) -> f64 {
    if headers.is_empty() {
        return 0.0;
    }

    let non_empty = headers
        .iter()
        .filter(|header| !header.trim().is_empty())
        .count() as f64;
    let ratio = non_empty / headers.len() as f64;
    (ratio * 10_000.0).round() / 10_000.0
}

fn resolve_source_kind(args: &EmitDiscoveryArgs) -> String {
    if let Some(source_kind) = args.source_kind.as_deref() {
        let normalized = normalize_source_kind(source_kind);
        if !normalized.is_empty() {
            return normalized;
        }
    }

    let from_path = args
        .source_file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(normalize_source_kind)
        .unwrap_or_default();
    if from_path.is_empty() {
        "source_file".to_string()
    } else {
        from_path
    }
}

fn normalize_source_kind(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_was_separator = false;

    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            previous_was_separator = false;
        } else if !previous_was_separator {
            normalized.push('_');
            previous_was_separator = true;
        }
    }

    let normalized = normalized.trim_matches('_').to_string();
    if normalized.is_empty() {
        return normalized;
    }
    if normalized
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic())
    {
        normalized
    } else {
        format!("source_{normalized}")
    }
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    format!("sha256:{digest:x}")
}
