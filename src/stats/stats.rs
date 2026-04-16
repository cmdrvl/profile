use std::collections::HashSet;
use std::fs::File;

use csv::StringRecord;
use serde_json::json;

use crate::cli::args::StatsArgs;
use crate::output::json::{CommandOutput, ProfileRef};
use crate::refusal::RefusalPayload;
use crate::schema::{
    ValidationMode, build_header_index, load_column_registry_aliases, parse_profile_yaml,
    resolve_registry_path, validate_profile,
};
use crate::witness::append::append_for_command;

const KEY_VIABLE_UNIQUENESS_THRESHOLD: f64 = 0.95;

#[derive(Debug, Default)]
struct ColumnAccumulator {
    null_count: usize,
    values: HashSet<String>,
    example: Option<String>,
}

struct SelectedColumns {
    columns: Vec<(String, usize)>,
    profile_ref: Option<ProfileRef>,
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

pub fn run(
    args: &StatsArgs,
    no_witness: bool,
    explicit: bool,
) -> Result<CommandOutput, RefusalPayload> {
    let file = File::open(&args.dataset).map_err(|error| {
        RefusalPayload::io(args.dataset.display().to_string(), error.to_string())
    })?;

    let mut reader = csv::Reader::from_reader(file);
    let headers = read_headers(&mut reader, &args.dataset.display().to_string())?;

    let selected = resolve_selected_columns(args, &headers)?;
    let selected_column_names = selected
        .columns
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();

    let mut accumulators = selected
        .columns
        .iter()
        .map(|_| ColumnAccumulator::default())
        .collect::<Vec<_>>();
    let mut row_count = 0usize;

    for record in reader.records() {
        let record = record.map_err(|error| {
            RefusalPayload::csv_parse(args.dataset.display().to_string(), error.to_string())
        })?;
        row_count += 1;
        apply_record(&record, &selected.columns, &mut accumulators);
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

            let mut col = json!({
                "name": name,
                "null_rate": null_rate,
                "uniqueness": uniqueness,
                "key_viable": key_viable,
            });

            if explicit {
                col["example"] = json!(accumulator.example.clone().unwrap_or_default());
            }

            col
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

    let witness_id = append_for_command(
        "stats",
        &result,
        inputs,
        json!({
            "subcommand": "stats",
            "profile": args.profile.as_ref().map(|path| path.display().to_string())
        }),
        no_witness,
    );

    Ok(CommandOutput::success(result)
        .with_profile_ref(selected.profile_ref)
        .with_witness_id(witness_id))
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
) -> Result<SelectedColumns, RefusalPayload> {
    if let Some(profile_path) = &args.profile {
        let profile_content = std::fs::read_to_string(profile_path).map_err(|error| {
            RefusalPayload::io(profile_path.display().to_string(), error.to_string())
        })?;

        let profile = parse_profile_yaml(&profile_content)?;
        validate_profile(&profile, ValidationMode::Validate)?;
        let profile_ref = ProfileRef::from_profile(&profile);
        let column_aliases = profile
            .column_registry
            .as_deref()
            .map(|registry| {
                load_column_registry_aliases(&resolve_registry_path(profile_path, registry))
            })
            .transpose()?;
        let index_by_name = build_header_index(headers, column_aliases.as_ref());

        let mut selected = Vec::with_capacity(profile.include_columns.len());
        let mut missing = Vec::new();

        for column in &profile.include_columns {
            if let Some(index) = index_by_name.column_index(column) {
                selected.push((column.clone(), index));
            } else {
                missing.push(column.clone());
            }
        }

        if !missing.is_empty() {
            return Err(RefusalPayload::column_not_found(
                missing,
                index_by_name.available(),
            ));
        }

        return Ok(SelectedColumns {
            columns: selected,
            profile_ref,
        });
    }

    Ok(SelectedColumns {
        columns: headers
            .iter()
            .enumerate()
            .map(|(index, name)| (name.to_string(), index))
            .collect::<Vec<_>>(),
        profile_ref: None,
    })
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
