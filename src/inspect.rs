//! Single-file, zero-retention dataset inspection.
//!
//! The inspector keeps values only long enough to calculate counts and numeric summaries. Its
//! default result contains no examples or raw string values, so it is safe to pass to an agent.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::Path;

use csv::StringRecord;
use serde_json::{Value, json};

use crate::cli::args::{InspectArgs, SuggestKeyArgs};
use crate::output::json::CommandOutput;
use crate::refusal::RefusalPayload;
use crate::schema::ColumnType;

#[derive(Debug, Default)]
struct ColumnAccumulator {
    null_count: usize,
    values: HashSet<String>,
    example: Option<String>,
    observed_types: [bool; 4],
    numeric_values: Vec<f64>,
    saw_non_numeric: bool,
}

impl ColumnAccumulator {
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
        let [integer, number, boolean, text] = self.observed_types;
        match (integer, number, boolean, text) {
            (true, false, false, false) => ColumnType::Integer,
            (true, true, false, false) | (false, true, false, false) => ColumnType::Number,
            (false, false, true, false) => ColumnType::Boolean,
            (false, false, false, true) => ColumnType::Text,
            (false, false, false, false) => ColumnType::Unknown,
            _ => ColumnType::Unknown,
        }
    }

    fn numeric_summary(&self) -> Value {
        if self.saw_non_numeric || self.numeric_values.is_empty() {
            return Value::Null;
        }
        let mut values = self.numeric_values.clone();
        values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
        json!({
            "min": values[0],
            "max": values[values.len() - 1],
            "p50": quantile(&values, 0.50),
            "p95": quantile(&values, 0.95),
        })
    }
}

pub fn run(args: &InspectArgs, explicit: bool) -> Result<CommandOutput, RefusalPayload> {
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

    let mut accumulators = (0..headers.len())
        .map(|_| ColumnAccumulator::default())
        .collect::<Vec<_>>();
    let mut row_count = 0usize;
    for record in reader.records() {
        let record = record.map_err(|error| {
            RefusalPayload::csv_parse(args.dataset.display().to_string(), error.to_string())
        })?;
        row_count += 1;
        observe_record(&record, &mut accumulators);
    }
    if row_count == 0 {
        return Err(RefusalPayload::empty_with_reason(
            args.dataset.display().to_string(),
            "no data rows",
        ));
    }

    let mut columns = Vec::with_capacity(headers.len());
    for (name, accumulator) in headers.iter().zip(&accumulators) {
        let null_rate = accumulator.null_count as f64 / row_count as f64;
        let mut column = json!({
            "name": name,
            "type": accumulator.inferred_type().as_str(),
            "null_count": accumulator.null_count,
            "null_rate": null_rate,
            "distinct_count": accumulator.values.len(),
            "uniqueness": accumulator.values.len() as f64 / row_count as f64,
            "numeric": accumulator.numeric_summary(),
        });
        if explicit {
            column["example"] = json!(accumulator.example.clone().unwrap_or_default());
        }
        columns.push(column);
    }

    let key_output = crate::stats::suggest_key::run(
        &SuggestKeyArgs {
            dataset: args.dataset.clone(),
            top: 5,
            max_arity: 3,
            sheet: args.sheet.clone(),
        },
        true,
    )?;
    let candidates = key_output
        .result
        .get("candidates")
        .cloned()
        .unwrap_or_else(|| json!([]));

    let (format, delimiter) = format_metadata(&args.dataset);
    let result = json!({
        "format": format,
        "dialect": {
            "name": format,
            "delimiter": delimiter,
            "source": "shared cmdrvl-tabular reader or profile CSV reader"
        },
        "row_count": row_count,
        "column_count": headers.len(),
        "columns": columns,
        "key_candidates": candidates,
        "suggestions": {
            "header_row": 1,
            "preamble_rows": 0,
            "confidence": "default",
            "note": "No preamble was inferred; use fingerprint peek for messy CSV row-shape analysis."
        }
    });

    Ok(CommandOutput::success(result))
}

fn observe_record(record: &StringRecord, accumulators: &mut [ColumnAccumulator]) {
    for (index, accumulator) in accumulators.iter_mut().enumerate() {
        accumulator.observe(record.get(index).unwrap_or_default());
    }
}

fn quantile(values: &[f64], probability: f64) -> f64 {
    let position = (values.len() - 1) as f64 * probability;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        values[lower]
    } else {
        values[lower] + (values[upper] - values[lower]) * (position - lower as f64)
    }
}

fn format_metadata(path: &Path) -> (&'static str, &'static str) {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("tsv") => ("tsv", "\\t"),
        Some("jsonl" | "ndjson") => ("jsonl", "none"),
        Some("parquet") => ("parquet", "none"),
        Some("xlsx" | "xlsm" | "xls") => ("xlsx", "none"),
        _ => ("csv", ","),
    }
}
