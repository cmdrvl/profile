use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use csv::StringRecord;
use serde::Deserialize;

use crate::refusal::RefusalPayload;
use crate::schema::profile::{Profile, ProfileStatus};

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
    read_registry_json_object(&registry_json_path)?;

    let mapping_paths = mapping_files(registry_dir)?;

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

pub fn registry_content_hash(registry_dir: &Path) -> Result<String, RefusalPayload> {
    if !registry_dir.exists() || !registry_dir.is_dir() {
        return Err(RefusalPayload::io(
            registry_dir.display().to_string(),
            "registry directory not found",
        ));
    }

    let mut framed = Vec::new();
    let registry_json_path = registry_dir.join("registry.json");
    let registry_json_bytes = read_registry_json_object(&registry_json_path)?;
    frame_registry_file("registry.json", &registry_json_bytes, &mut framed);

    for path in mapping_files(registry_dir)? {
        let relative_path = registry_relative_file_name(&path)?;
        let bytes = fs::read(&path)
            .map_err(|error| RefusalPayload::io(path.display().to_string(), error.to_string()))?;
        frame_registry_file(&relative_path, &bytes, &mut framed);
    }

    Ok(format!("blake3:{}", blake3::hash(&framed).to_hex()))
}

pub fn validate_frozen_registry_hash(
    profile_path: &Path,
    profile: &Profile,
) -> Result<(), RefusalPayload> {
    if !matches!(profile.status, ProfileStatus::Frozen) {
        return Ok(());
    }

    let Some(registry_ref) = profile.column_registry.as_deref() else {
        return Ok(());
    };
    let declared = profile
        .column_registry_hash
        .as_deref()
        .ok_or_else(|| RefusalPayload::missing_field("column_registry_hash"))?;
    let resolved_registry = resolve_registry_path(profile_path, registry_ref);
    let computed = registry_content_hash(&resolved_registry)?;

    if computed != declared {
        return Err(RefusalPayload::invalid_schema_single(
            "column_registry_hash",
            format!("registry content hash drift: declared {declared}, computed {computed}"),
        ));
    }

    Ok(())
}

fn mapping_files(registry_dir: &Path) -> Result<Vec<PathBuf>, RefusalPayload> {
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
    mapping_paths.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
    Ok(mapping_paths)
}

fn read_registry_json_object(path: &Path) -> Result<Vec<u8>, RefusalPayload> {
    let bytes = fs::read(path)
        .map_err(|error| RefusalPayload::io(path.display().to_string(), error.to_string()))?;
    let value = serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|error| {
        RefusalPayload::invalid_schema_single(
            "column_registry",
            format!(
                "failed to parse registry definition '{}': {error}",
                path.display()
            ),
        )
    })?;
    if !value.is_object() {
        return Err(RefusalPayload::invalid_schema_single(
            "column_registry",
            format!(
                "registry definition '{}' must be a JSON object",
                path.display()
            ),
        ));
    }
    Ok(bytes)
}

fn registry_relative_file_name(path: &Path) -> Result<String, RefusalPayload> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            RefusalPayload::invalid_schema_single(
                "column_registry",
                format!("registry file '{}' is not valid UTF-8", path.display()),
            )
        })
}

fn frame_registry_file(relative_path: &str, bytes: &[u8], framed: &mut Vec<u8>) {
    framed.extend_from_slice(relative_path.as_bytes());
    framed.push(0);
    framed.extend_from_slice(bytes.len().to_string().as_bytes());
    framed.push(0);
    framed.extend_from_slice(bytes);
    framed.push(0xFF);
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
