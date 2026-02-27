use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs::File;
use std::path::Path;

use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::cli::args::SuggestKeyArgs;
use crate::refusal::RefusalPayload;
use crate::witness::append::append_for_command;

const AUTO_KEY_UNIQUENESS_THRESHOLD: f64 = 0.95;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyCandidate {
    pub column: String,
    pub position: usize,
    pub uniqueness: f64,
    pub null_rate: f64,
    pub stability_score: u8,
    pub viable: bool,
    pub rank: usize,
}

#[derive(Debug, Default)]
struct ColumnStats {
    unique_values: HashSet<String>,
    null_count: usize,
}

impl ColumnStats {
    fn observe(&mut self, value: &str) {
        if value.trim().is_empty() {
            self.null_count += 1;
        } else {
            self.unique_values.insert(value.to_string());
        }
    }
}

pub fn run(args: &SuggestKeyArgs, no_witness: bool) -> Result<Value, RefusalPayload> {
    let file = File::open(&args.dataset).map_err(|error| {
        RefusalPayload::io(args.dataset.display().to_string(), error.to_string())
    })?;

    let mut reader = ReaderBuilder::new().has_headers(true).from_reader(file);
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

    let header_names = headers
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();
    let mut candidates = analyze_columns(&mut reader, &header_names, &args.dataset)?;

    candidates.sort_by(|left, right| {
        right
            .uniqueness
            .partial_cmp(&left.uniqueness)
            .unwrap_or(Ordering::Equal)
            .then(
                left.null_rate
                    .partial_cmp(&right.null_rate)
                    .unwrap_or(Ordering::Equal),
            )
            .then(right.stability_score.cmp(&left.stability_score))
            .then(left.position.cmp(&right.position))
    });

    for (index, candidate) in candidates.iter_mut().enumerate() {
        candidate.rank = index + 1;
    }

    let top_candidates = candidates
        .into_iter()
        .take(args.top)
        .collect::<Vec<KeyCandidate>>();

    let result = json!({
        "candidates": top_candidates,
        "top": args.top,
        "ranking": {
            "order": [
                "uniqueness_desc",
                "null_rate_asc",
                "stability_desc",
                "position_asc"
            ],
            "stability_signals": ["*_id", "*_key", "*_number"]
        }
    });

    append_for_command(
        "suggest-key",
        &result,
        vec![args.dataset.clone()],
        json!({
            "subcommand": "suggest-key",
            "top": args.top
        }),
        no_witness,
    );

    Ok(result)
}

fn analyze_columns(
    reader: &mut csv::Reader<File>,
    headers: &[String],
    dataset_path: &Path,
) -> Result<Vec<KeyCandidate>, RefusalPayload> {
    let mut stats = headers
        .iter()
        .map(|_| ColumnStats::default())
        .collect::<Vec<_>>();
    let mut total_rows = 0usize;

    for record in reader.records() {
        let record = record.map_err(|error| {
            RefusalPayload::csv_parse(dataset_path.display().to_string(), error.to_string())
        })?;
        total_rows += 1;

        for (index, _) in headers.iter().enumerate() {
            let value = record.get(index).unwrap_or_default();
            stats[index].observe(value);
        }
    }

    if total_rows == 0 {
        return Err(RefusalPayload::empty_with_reason(
            dataset_path.display().to_string(),
            "no data rows",
        ));
    }

    Ok(headers
        .iter()
        .enumerate()
        .map(|(position, column_name)| {
            let column_stats = &stats[position];
            let uniqueness = column_stats.unique_values.len() as f64 / total_rows as f64;
            let null_rate = column_stats.null_count as f64 / total_rows as f64;
            let stability_score = stability_score(column_name);
            let viable = uniqueness >= AUTO_KEY_UNIQUENESS_THRESHOLD && null_rate == 0.0;

            KeyCandidate {
                column: column_name.clone(),
                position,
                uniqueness,
                null_rate,
                stability_score,
                viable,
                rank: 0,
            }
        })
        .collect::<Vec<_>>())
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
