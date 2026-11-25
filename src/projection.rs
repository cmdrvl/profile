//! Deterministic projections from a frozen profile. These projections deliberately consume
//! profile metadata and optional redacted stats only; raw cell values never enter the output.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::cli::args::{DdlDialect, EmitDdlArgs, EmitVerifyStarterArgs};
use crate::output::json::{CommandOutput, ProfileRef};
use crate::refusal::RefusalPayload;
use crate::schema::{
    ColumnMetadata, ColumnType, Profile, ValidationMode, parse_profile_yaml, validate_profile,
};

pub fn emit_verify_starter(
    args: &EmitVerifyStarterArgs,
    no_witness: bool,
) -> Result<CommandOutput, RefusalPayload> {
    let profile = load_frozen_profile(&args.profile)?;
    let stats = load_stats(args.stats.as_deref())?;
    let columns = merged_columns(&profile, &stats);
    let key = profile.key.clone();
    let rules = columns
        .iter()
        .map(|(name, metadata)| {
            let mut checks = Vec::new();
            if metadata.nullable == Some(false) {
                checks.push("not_null");
            }
            if key.iter().any(|column| column == name) {
                checks.push("unique");
            }
            let mut rule = json!({
                "column": name,
                "type": metadata.type_.unwrap_or(ColumnType::Unknown).as_str(),
                "nullable": metadata.nullable.unwrap_or(true),
                "checks": checks,
            });
            add_optional_metadata(&mut rule, metadata);
            rule
        })
        .collect::<Vec<_>>();

    let mut result = json!({
        "schema": "verify.rules.v1",
        "profile_id": profile.profile_id,
        "profile_sha256": profile.profile_sha256,
        "key": key,
        "rules": rules,
    });
    if let Some(path) = args.out.as_deref() {
        let bytes = serde_json::to_vec_pretty(&result).map_err(|error| {
            RefusalPayload::invalid_schema_single("emit-verify-starter", error.to_string())
        })?;
        fs::write(path, bytes)
            .map_err(|error| RefusalPayload::io(path.display().to_string(), error.to_string()))?;
        result["path"] = json!(path.display().to_string());
    }

    let _ = no_witness;
    Ok(CommandOutput::success(result).with_profile_ref(ProfileRef::from_profile(&profile)))
}

pub fn emit_ddl(args: &EmitDdlArgs, no_witness: bool) -> Result<CommandOutput, RefusalPayload> {
    let profile = load_frozen_profile(&args.profile)?;
    let stats = load_stats(args.stats.as_deref())?;
    let columns = merged_columns(&profile, &stats);
    let table = args
        .table
        .clone()
        .or_else(|| profile.profile_family.clone())
        .or_else(|| profile.profile_id.clone())
        .unwrap_or_else(|| "profile_data".to_owned());
    if table.trim().is_empty() {
        return Err(RefusalPayload::invalid_schema_single(
            "table",
            "must be non-empty when set",
        ));
    }

    let definitions = columns
        .iter()
        .map(|(name, metadata)| {
            let sql_type = sql_type(metadata.type_.unwrap_or(ColumnType::Unknown), args.dialect);
            let nullable = if metadata.nullable == Some(false) {
                " NOT NULL"
            } else {
                ""
            };
            format!("  {} {sql_type}{nullable}", quote_identifier(name))
        })
        .collect::<Vec<_>>();
    let sql = format!(
        "CREATE TABLE {} (\n{}\n);\n",
        quote_identifier(&table),
        definitions.join(",\n")
    );
    let mut result = json!({
        "schema": "twinning.ddl.v1",
        "dialect": match args.dialect {
            DdlDialect::Postgres => "postgres",
            DdlDialect::Snowflake => "snowflake",
        },
        "profile_id": profile.profile_id,
        "profile_sha256": profile.profile_sha256,
        "table": table,
        "columns": columns.iter().map(|(name, metadata)| json!({
            "name": name,
            "type": sql_type(metadata.type_.unwrap_or(ColumnType::Unknown), args.dialect),
            "nullable": metadata.nullable.unwrap_or(true),
        })).collect::<Vec<_>>(),
        "sql": sql,
    });
    if let Some(path) = args.out.as_deref() {
        fs::write(path, result["sql"].as_str().unwrap_or_default())
            .map_err(|error| RefusalPayload::io(path.display().to_string(), error.to_string()))?;
        result["path"] = json!(path.display().to_string());
    }

    let _ = no_witness;
    Ok(CommandOutput::success(result).with_profile_ref(ProfileRef::from_profile(&profile)))
}

fn load_frozen_profile(path: &Path) -> Result<Profile, RefusalPayload> {
    let content = fs::read_to_string(path)
        .map_err(|error| RefusalPayload::io(path.display().to_string(), error.to_string()))?;
    let profile = parse_profile_yaml(&content)?;
    if !profile.is_frozen() {
        return Err(RefusalPayload::invalid_schema_single(
            "profile.status",
            "projection exports require a frozen profile",
        ));
    }
    validate_profile(&profile, ValidationMode::Validate)?;
    Ok(profile)
}

fn load_stats(path: Option<&Path>) -> Result<BTreeMap<String, ColumnMetadata>, RefusalPayload> {
    let Some(path) = path else {
        return Ok(BTreeMap::new());
    };
    let content = fs::read_to_string(path)
        .map_err(|error| RefusalPayload::io(path.display().to_string(), error.to_string()))?;
    let value: Value = serde_json::from_str(&content).map_err(|error| {
        RefusalPayload::invalid_schema_single("stats", format!("stats JSON is invalid: {error}"))
    })?;
    let columns = value
        .get("result")
        .and_then(|result| result.get("columns"))
        .or_else(|| value.get("columns"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RefusalPayload::invalid_schema_single(
                "stats",
                "stats JSON must contain a result.columns array",
            )
        })?;
    let mut inferred = BTreeMap::new();
    for column in columns {
        let Some(name) = column.get("name").and_then(Value::as_str) else {
            // bd-ds5: default `profile stats` withholds dataset header text (columns carry an
            // `id`, not a `name`). Silently skipping them would drop every inferred type.
            return Err(RefusalPayload::invalid_schema_single(
                "stats",
                "stats JSON columns carry no names (redacted output); rerun profile stats with --profile <frozen profile> or --explicit",
            ));
        };
        let type_ = column
            .get("type")
            .and_then(Value::as_str)
            .and_then(parse_column_type);
        let nullable = column.get("nullable").and_then(Value::as_bool);
        inferred.insert(
            name.to_owned(),
            ColumnMetadata {
                type_,
                nullable,
                ..ColumnMetadata::default()
            },
        );
    }
    Ok(inferred)
}

fn merged_columns(
    profile: &Profile,
    stats: &BTreeMap<String, ColumnMetadata>,
) -> Vec<(String, ColumnMetadata)> {
    profile
        .include_columns
        .iter()
        .map(|name| {
            let mut metadata = profile.columns.get(name).cloned().unwrap_or_default();
            if let Some(inferred) = stats.get(name) {
                if metadata.type_.is_none() {
                    metadata.type_ = inferred.type_;
                }
                if metadata.nullable.is_none() {
                    metadata.nullable = inferred.nullable;
                }
            }
            (name.clone(), metadata)
        })
        .collect()
}

fn parse_column_type(value: &str) -> Option<ColumnType> {
    Some(match value {
        "integer" => ColumnType::Integer,
        "number" => ColumnType::Number,
        "boolean" => ColumnType::Boolean,
        "text" => ColumnType::Text,
        "date" => ColumnType::Date,
        "unknown" => ColumnType::Unknown,
        _ => return None,
    })
}

fn add_optional_metadata(value: &mut Value, metadata: &ColumnMetadata) {
    if let Some(unit) = metadata.unit.as_deref() {
        value["unit"] = json!(unit);
    }
    if let Some(sensitivity) = metadata.sensitivity {
        value["sensitivity"] = json!(sensitivity.as_str());
    }
    if let Some(airlock_class) = metadata.airlock_class {
        value["airlock_class"] = json!(airlock_class.as_str());
    }
}

fn sql_type(column_type: ColumnType, dialect: DdlDialect) -> &'static str {
    match (column_type, dialect) {
        (ColumnType::Integer, DdlDialect::Postgres) => "BIGINT",
        (ColumnType::Number, DdlDialect::Postgres) => "DOUBLE PRECISION",
        (ColumnType::Boolean, _) => "BOOLEAN",
        (ColumnType::Date, _) => "DATE",
        (ColumnType::Integer, DdlDialect::Snowflake) => "NUMBER(38,0)",
        (ColumnType::Number, DdlDialect::Snowflake) => "FLOAT",
        (ColumnType::Text | ColumnType::Unknown, _) => "TEXT",
    }
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
