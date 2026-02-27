use std::collections::{HashMap, HashSet};
use std::fs::File;

use csv::StringRecord;
use serde_json::{Value, json};

use crate::cli::args::StatsArgs;
use crate::refusal::RefusalPayload;
use crate::schema::{ValidationMode, parse_profile_yaml, validate_profile};
use crate::witness::append::append_for_command;

const KEY_VIABLE_UNIQUENESS_THRESHOLD: f64 = 0.95;

#[derive(Debug, Default)]
struct ColumnAccumulator {
    null_count: usize,
    values: HashSet<String>,
    example: Option<String>,
}

impl ColumnAccumulator {
    fn observe(&mut self, value: &str) {
        if value.trim().is_empty() {
            self.null_count += 1;
            return;
        }

        if self.example.is_none() {
            self.example = Some(value.to_string());
        }

        self.values.insert(value.to_string());
    }
}

pub fn run(args: &StatsArgs, no_witness: bool) -> Result<Value, RefusalPayload> {
    let file = File::open(&args.dataset).map_err(|error| {
        RefusalPayload::io(args.dataset.display().to_string(), error.to_string())
    })?;

    let mut reader = csv::Reader::from_reader(file);
    let headers = read_headers(&mut reader, &args.dataset.display().to_string())?;

    let selected_columns = resolve_selected_columns(args, &headers)?;
    let selected_column_names = selected_columns
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();

    let mut accumulators = selected_columns
        .iter()
        .map(|_| ColumnAccumulator::default())
        .collect::<Vec<_>>();
    let mut row_count = 0usize;

    for record in reader.records() {
        let record = record.map_err(|error| {
            RefusalPayload::csv_parse(args.dataset.display().to_string(), error.to_string())
        })?;
        row_count += 1;
        apply_record(&record, &selected_columns, &mut accumulators);
    }

    if row_count == 0 {
        return Err(RefusalPayload::empty_with_reason(
            args.dataset.display().to_string(),
            "no data rows",
        ));
    }

    let columns = selected_column_names
        .iter()
        .zip(accumulators.iter())
        .map(|(name, accumulator)| {
            let null_rate = accumulator.null_count as f64 / row_count as f64;
            let uniqueness = accumulator.values.len() as f64 / row_count as f64;
            let key_viable = null_rate == 0.0 && uniqueness >= KEY_VIABLE_UNIQUENESS_THRESHOLD;

            json!({
                "name": name,
                "null_rate": null_rate,
                "uniqueness": uniqueness,
                "key_viable": key_viable,
                "example": accumulator.example.clone().unwrap_or_default()
            })
        })
        .collect::<Vec<_>>();

    let result = json!({
        "row_count": row_count,
        "column_count": selected_column_names.len(),
        "columns": columns
    });

    let mut inputs = vec![args.dataset.clone()];
    if let Some(profile) = &args.profile {
        inputs.push(profile.clone());
    }

    append_for_command(
        "stats",
        &result,
        inputs,
        json!({
            "subcommand": "stats",
            "profile": args.profile.as_ref().map(|path| path.display().to_string())
        }),
        no_witness,
    );

    Ok(result)
}

fn read_headers(
    reader: &mut csv::Reader<File>,
    dataset_path: &str,
) -> Result<StringRecord, RefusalPayload> {
    let headers = reader
        .headers()
        .map_err(|error| RefusalPayload::csv_parse(dataset_path.to_string(), error.to_string()))?
        .clone();

    if headers.is_empty() {
        return Err(RefusalPayload::empty_with_reason(
            dataset_path.to_string(),
            "no header row",
        ));
    }

    Ok(headers)
}

fn resolve_selected_columns(
    args: &StatsArgs,
    headers: &StringRecord,
) -> Result<Vec<(String, usize)>, RefusalPayload> {
    let index_by_name = headers
        .iter()
        .enumerate()
        .map(|(index, name)| (name.to_string(), index))
        .collect::<HashMap<_, _>>();

    if let Some(profile_path) = &args.profile {
        let profile_content = std::fs::read_to_string(profile_path).map_err(|error| {
            RefusalPayload::io(profile_path.display().to_string(), error.to_string())
        })?;

        let profile = parse_profile_yaml(&profile_content)?;
        validate_profile(&profile, ValidationMode::Validate)?;

        let mut selected = Vec::with_capacity(profile.include_columns.len());
        let mut missing = Vec::new();

        for column in &profile.include_columns {
            if let Some(index) = index_by_name.get(column).copied() {
                selected.push((column.clone(), index));
            } else {
                missing.push(column.clone());
            }
        }

        if !missing.is_empty() {
            let available = headers
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>();
            return Err(RefusalPayload::column_not_found(missing, available));
        }

        return Ok(selected);
    }

    Ok(headers
        .iter()
        .enumerate()
        .map(|(index, name)| (name.to_string(), index))
        .collect::<Vec<_>>())
}

fn apply_record(
    record: &StringRecord,
    selected_columns: &[(String, usize)],
    accumulators: &mut [ColumnAccumulator],
) {
    for (position, (_, index)) in selected_columns.iter().enumerate() {
        let value = record.get(*index).unwrap_or_default();
        accumulators[position].observe(value);
    }
}
