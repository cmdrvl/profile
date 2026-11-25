use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::cli::args::SuggestKeyArgs;
use crate::output::json::CommandOutput;
use crate::redaction::{Kind, Marker};
use crate::refusal::RefusalPayload;
use crate::witness::append::append_for_command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyCandidate {
    /// Header positions of `columns`; orders ties by position, never by header text.
    #[serde(skip)]
    pub indices: Vec<usize>,
    pub column: String,
    pub columns: Vec<String>,
    pub position: usize,
    pub arity: usize,
    pub uniqueness: f64,
    pub null_rate: f64,
    pub stability_score: u8,
    pub viable: bool,
    pub duplicate_count: usize,
    pub incomplete_count: usize,
    pub whitespace_collision_count: usize,
    pub case_collision_count: usize,
    pub rank: usize,
}

type Hash128 = [u8; 16];

const AUTO_KEY_UNIQUENESS_THRESHOLD: f64 = 0.95;
const MAX_COMPOSITE_ARITY: usize = 3;
const NORMALIZED_SEEN: u8 = 1;
const TRIMMED_SEEN: u8 = 2;
const PADDED_SEEN: u8 = 4;
const LOWER_SEEN: u8 = 1;
const NON_LOWER_SEEN: u8 = 2;

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
        if *state & NORMALIZED_SEEN != 0 {
            self.duplicate_count += 1;
        }
        if padded && *state & TRIMMED_SEEN != 0 {
            self.whitespace_collision_count += 1;
        }
        if !padded && *state & PADDED_SEEN != 0 {
            self.whitespace_collision_count += 1;
        }
        *state |= NORMALIZED_SEEN;
        *state |= if padded { PADDED_SEEN } else { TRIMMED_SEEN };

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
        if all_lower && *lower_state & NON_LOWER_SEEN != 0 {
            self.case_collision_count += 1;
        }
        if !all_lower && *lower_state & LOWER_SEEN != 0 {
            self.case_collision_count += 1;
        }
        *lower_state |= if all_lower {
            LOWER_SEEN
        } else {
            NON_LOWER_SEEN
        };
    }

    fn unique_count(&self) -> usize {
        self.normalized.len()
    }
}

pub fn run(
    args: &SuggestKeyArgs,
    no_witness: bool,
    explicit: bool,
) -> Result<CommandOutput, RefusalPayload> {
    let mut marker = Marker::for_output(explicit)?;
    let mut reader = crate::tabular_input::open_dataset(&args.dataset, args.sheet.as_deref())?;
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
    let mut candidates =
        analyze_columns(&mut reader, &header_names, &args.dataset, args.max_arity)?;

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
            .then(left.arity.cmp(&right.arity))
            .then(left.position.cmp(&right.position))
            .then(left.indices.cmp(&right.indices))
    });

    for (index, candidate) in candidates.iter_mut().enumerate() {
        candidate.rank = index + 1;
    }

    let mut top_candidates = candidates
        .into_iter()
        .take(args.top)
        .collect::<Vec<KeyCandidate>>();

    // Header text can be a banner or a data row on a messy sheet: without --explicit a column
    // is its ordinal ref (numbered in header order, never in text order) or keyed marker.
    // The ranking above never reads the text except through `stability_score`'s name suffix
    // test, which is a derived number.
    if !explicit {
        let refs = header_names
            .iter()
            .map(|name| marker.mark(Kind::Column, name))
            .collect::<Vec<_>>();
        for candidate in &mut top_candidates {
            candidate.columns = candidate.indices.iter().map(|i| refs[*i].clone()).collect();
            candidate.column = candidate.columns.join(" + ");
        }
    }

    let result = json!({
        "candidates": top_candidates,
        "redaction": if explicit { "explicit" } else { marker.mode() },
        "top": args.top,
        "max_arity": args.max_arity,
        "ranking": {
            "order": [
                "uniqueness_desc",
                "null_rate_asc",
                "stability_desc",
                "arity_asc",
                "position_asc"
            ],
            "stability_signals": ["*_id", "*_key", "*_number"],
            "composite_semantics": "ordered tuples; ASCII-trim each component; literal NA/NULL remain values"
        }
    });

    let witness_id = append_for_command(
        "suggest-key",
        &result,
        vec![args.dataset.clone()],
        json!({
            "subcommand": "suggest-key",
            "top": args.top,
            "max_arity": args.max_arity
        }),
        no_witness,
    );

    Ok(CommandOutput::success(result).with_witness_id(witness_id))
}

fn analyze_columns(
    reader: &mut csv::Reader<Box<dyn std::io::Read>>,
    headers: &[String],
    dataset_path: &Path,
    requested_max_arity: usize,
) -> Result<Vec<KeyCandidate>, RefusalPayload> {
    let evaluation_max_arity = if requested_max_arity == 1 {
        MAX_COMPOSITE_ARITY
    } else {
        requested_max_arity
    };
    let specs = candidate_specs(headers.len(), evaluation_max_arity);
    let mut stats = specs
        .iter()
        .map(|_| CandidateStats::default())
        .collect::<Vec<_>>();
    let mut total_rows = 0usize;

    for record in reader.records() {
        let record = record.map_err(|error| {
            RefusalPayload::csv_parse(dataset_path.display().to_string(), error.to_string())
        })?;
        total_rows += 1;

        for (spec, candidate_stats) in specs.iter().zip(&mut stats) {
            let values = spec
                .iter()
                .map(|index| record.get(*index).unwrap_or_default())
                .collect::<Vec<_>>();
            candidate_stats.observe(&values);
        }
    }

    if total_rows == 0 {
        return Err(RefusalPayload::empty_with_reason(
            dataset_path.display().to_string(),
            "no data rows",
        ));
    }

    let single_candidates = specs
        .iter()
        .zip(&stats)
        .filter(|(spec, _)| spec.len() == 1)
        .collect::<Vec<_>>();
    let has_perfect_single = single_candidates.iter().any(|(_, candidate_stats)| {
        candidate_stats.incomplete_count == 0 && candidate_stats.unique_count() == total_rows
    });
    let include_composites = requested_max_arity > 1 || !has_perfect_single;

    Ok(specs
        .iter()
        .zip(stats)
        .filter(|(spec, _)| spec.len() == 1 || include_composites)
        .map(|(spec, candidate_stats)| {
            candidate_from_stats(spec, candidate_stats, headers, total_rows)
        })
        .collect())
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
    total_rows: usize,
) -> KeyCandidate {
    let columns = spec
        .iter()
        .map(|index| headers[*index].clone())
        .collect::<Vec<_>>();
    let uniqueness = stats.unique_count() as f64 / total_rows as f64;
    let null_rate = stats.incomplete_count as f64 / total_rows as f64;
    let stability_score = columns
        .iter()
        .map(|column| stability_score(column))
        .sum::<u8>()
        .min(3);
    let viable = uniqueness >= AUTO_KEY_UNIQUENESS_THRESHOLD && stats.incomplete_count == 0;

    KeyCandidate {
        indices: spec.to_vec(),
        column: columns.join(" + "),
        columns,
        position: spec[0],
        arity: spec.len(),
        uniqueness,
        null_rate,
        stability_score,
        viable,
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
