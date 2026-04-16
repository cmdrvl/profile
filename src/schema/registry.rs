use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use csv::StringRecord;
use serde::Deserialize;

use crate::refusal::RefusalPayload;

const COLUMN_NAME_CANONICAL_TYPE: &str = "column_name";

#[derive(Debug, Deserialize)]
struct MappingEntry {
    input: String,
    canonical_id: String,
    canonical_type: String,
    rule_id: String,
}

#[derive(Debug, Clone)]
pub struct HeaderIndex {
    lookup: HashMap<String, usize>,
    available: Vec<String>,
}

impl HeaderIndex {
    pub fn column_index(&self, column: &str) -> Option<usize> {
        self.lookup.get(column).copied()
    }

    pub fn available(&self) -> Vec<String> {
        self.available.clone()
    }
}

pub fn resolve_registry_path(anchor_path: &Path, registry_ref: &str) -> PathBuf {
    let registry_path = Path::new(registry_ref);
    if registry_path.is_absolute() {
        registry_path.to_path_buf()
    } else {
        anchor_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(registry_path)
    }
}

pub fn load_column_registry_aliases(
    registry_dir: &Path,
) -> Result<HashMap<String, String>, RefusalPayload> {
    if !registry_dir.exists() || !registry_dir.is_dir() {
        return Err(RefusalPayload::io(
            registry_dir.display().to_string(),
            "registry directory not found",
        ));
    }

    let registry_json_path = registry_dir.join("registry.json");
    let registry_json = fs::read_to_string(&registry_json_path).map_err(|error| {
        RefusalPayload::io(registry_json_path.display().to_string(), error.to_string())
    })?;
    serde_json::from_str::<serde_json::Value>(&registry_json).map_err(|error| {
        RefusalPayload::invalid_schema_single(
            "column_registry",
            format!(
                "failed to parse registry definition '{}': {error}",
                registry_json_path.display()
            ),
        )
    })?;

    let mut mapping_paths = fs::read_dir(registry_dir)
        .map_err(|error| RefusalPayload::io(registry_dir.display().to_string(), error.to_string()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file()
                && path.extension().is_some_and(|ext| ext == "json")
                && path.file_name() != Some("registry.json".as_ref())
                && path.file_name() != Some("_build.json".as_ref())
        })
        .collect::<Vec<_>>();
    mapping_paths.sort();

    let mut aliases = HashMap::new();
    for path in mapping_paths {
        let content = fs::read_to_string(&path)
            .map_err(|error| RefusalPayload::io(path.display().to_string(), error.to_string()))?;
        let entries: Vec<MappingEntry> = serde_json::from_str(&content).map_err(|error| {
            RefusalPayload::invalid_schema_single(
                "column_registry",
                format!("failed to parse mapping file '{}': {error}", path.display()),
            )
        })?;

        for (index, entry) in entries.into_iter().enumerate() {
            if entry.input.trim().is_empty()
                || entry.canonical_id.trim().is_empty()
                || entry.canonical_type.trim().is_empty()
                || entry.rule_id.trim().is_empty()
            {
                return Err(RefusalPayload::invalid_schema_single(
                    "column_registry",
                    format!(
                        "invalid mapping entry {index} in '{}': missing required fields",
                        path.display()
                    ),
                ));
            }

            if entry.canonical_type == COLUMN_NAME_CANONICAL_TYPE {
                aliases.entry(entry.input).or_insert(entry.canonical_id);
            }
        }
    }

    Ok(aliases)
}

pub fn canonicalize_profile_column(
    column: &str,
    aliases: Option<&HashMap<String, String>>,
) -> String {
    aliases
        .and_then(|aliases| aliases.get(column))
        .cloned()
        .unwrap_or_else(|| column.to_string())
}

pub fn canonicalize_header_sequence(
    headers: &StringRecord,
    aliases: Option<&HashMap<String, String>>,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut columns = Vec::new();

    for header in headers.iter() {
        let resolved = canonicalize_profile_column(header, aliases);
        if seen.insert(resolved.clone()) {
            columns.push(resolved);
        }
    }

    columns
}

pub fn build_header_index(
    headers: &StringRecord,
    aliases: Option<&HashMap<String, String>>,
) -> HeaderIndex {
    let mut lookup = HashMap::new();
    let mut available = Vec::new();
    let mut seen_available = HashSet::new();

    for (index, header) in headers.iter().enumerate() {
        let raw = header.to_string();
        if seen_available.insert(raw.clone()) {
            available.push(raw.clone());
        }
        lookup.entry(raw.clone()).or_insert(index);

        if let Some(canonical) = aliases.and_then(|aliases| aliases.get(header)).cloned() {
            if seen_available.insert(canonical.clone()) {
                available.push(canonical.clone());
            }
            lookup.entry(canonical).or_insert(index);
        }
    }

    HeaderIndex { lookup, available }
}
