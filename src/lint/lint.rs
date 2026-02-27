use std::collections::HashSet;
use std::fs;
use std::fs::File;

use serde_json::{Value, json};

use crate::cli::args::LintArgs;
use crate::refusal::RefusalPayload;
use crate::schema::{ValidationMode, parse_profile_yaml, validate_profile};
use crate::witness::append::append_for_command;

pub fn run(args: &LintArgs, no_witness: bool) -> Result<Value, RefusalPayload> {
    let profile_content = fs::read_to_string(&args.profile).map_err(|error| {
        RefusalPayload::io(args.profile.display().to_string(), error.to_string())
    })?;
    let profile = parse_profile_yaml(&profile_content)?;
    validate_profile(&profile, ValidationMode::Validate)?;

    let file = File::open(&args.against).map_err(|error| {
        RefusalPayload::io(args.against.display().to_string(), error.to_string())
    })?;
    let mut reader = csv::Reader::from_reader(file);

    let headers = reader
        .headers()
        .map_err(|error| {
            RefusalPayload::csv_parse(args.against.display().to_string(), error.to_string())
        })?
        .clone();
    if headers.is_empty() {
        return Err(RefusalPayload::empty_with_reason(
            args.against.display().to_string(),
            "no header row",
        ));
    }

    let available = headers.iter().collect::<HashSet<_>>();
    let mut issues = Vec::new();

    for column in &profile.include_columns {
        if !available.contains(column.as_str()) {
            issues.push(json!({
                "kind": "missing_column",
                "column": column,
                "severity": "error"
            }));
        }
    }

    for column in &profile.key {
        if !available.contains(column.as_str()) {
            issues.push(json!({
                "kind": "missing_key",
                "column": column,
                "severity": "error"
            }));
        }
    }

    let result = json!({ "issues": issues });
    append_for_command(
        "lint",
        &result,
        vec![args.profile.clone(), args.against.clone()],
        json!({
            "subcommand": "lint",
            "against": args.against.display().to_string()
        }),
        no_witness,
    );

    Ok(result)
}
