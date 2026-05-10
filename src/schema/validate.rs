use crate::refusal::RefusalPayload;
use crate::schema::profile::{
    HashAlgorithm, HeaderMergeStrategy, Profile, ProfileStatus, SliceMode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    Validate,
    Freeze,
}

pub fn parse_profile_yaml(content: &str) -> Result<Profile, RefusalPayload> {
    Profile::from_yaml(content).map_err(map_yaml_error)
}

pub fn validate_profile(profile: &Profile, mode: ValidationMode) -> Result<(), RefusalPayload> {
    if profile.schema_version != 1 {
        return Err(invalid_schema("schema_version", "must be 1"));
    }

    if !matches!(profile.format, crate::schema::ProfileFormat::Csv) {
        return Err(invalid_schema("format", "must be csv"));
    }

    if profile
        .column_registry
        .as_deref()
        .is_some_and(|registry| registry.trim().is_empty())
    {
        return Err(invalid_schema(
            "column_registry",
            "must be a non-empty path when set",
        ));
    }

    if profile
        .fingerprint_ref
        .as_deref()
        .is_some_and(|fingerprint_ref| fingerprint_ref.trim().is_empty())
    {
        return Err(invalid_schema(
            "fingerprint_ref",
            "must be a non-empty fingerprint ID when set",
        ));
    }

    validate_pre_parse(profile)?;

    if profile.key.iter().any(|column| column.trim().is_empty()) {
        return Err(invalid_schema("key", "columns must be non-empty strings"));
    }

    if profile
        .include_columns
        .iter()
        .any(|column| column.trim().is_empty())
    {
        return Err(invalid_schema(
            "include_columns",
            "entries must be non-empty strings",
        ));
    }

    if matches!(mode, ValidationMode::Freeze) && profile.include_columns.is_empty() {
        return Err(invalid_schema(
            "include_columns",
            "must be non-empty for freeze",
        ));
    }

    if profile.is_frozen() {
        let family = profile
            .profile_family
            .as_deref()
            .ok_or_else(|| missing_field("profile_family"))?;
        let version = profile
            .profile_version
            .ok_or_else(|| missing_field("profile_version"))?;
        let profile_id = profile
            .profile_id
            .as_deref()
            .ok_or_else(|| missing_field("profile_id"))?;

        if !is_valid_profile_family(family) {
            return Err(invalid_schema(
                "profile_family",
                "profile_family is not in valid family format",
            ));
        }

        let expected_id = format!("{family}.v{version}");
        if profile_id != expected_id {
            return Err(invalid_schema(
                "profile_id",
                "profile_id must equal <profile_family>.v<profile_version>",
            ));
        }

        let profile_sha256 = profile.profile_sha256.as_deref();
        if matches!(mode, ValidationMode::Validate) {
            let sha = profile_sha256.ok_or_else(|| missing_field("profile_sha256"))?;

            if !is_valid_profile_sha256(sha) {
                return Err(invalid_schema(
                    "profile_sha256",
                    "profile_sha256 must match sha256:<64 lowercase hex chars>",
                ));
            }
        } else if let Some(sha) = profile_sha256
            && !is_valid_profile_sha256(sha)
        {
            return Err(invalid_schema(
                "profile_sha256",
                "profile_sha256 must match sha256:<64 lowercase hex chars>",
            ));
        }

        // Status is already validated to be frozen if we get here
    } else if matches!(profile.status, ProfileStatus::Draft)
        && (profile.profile_id.is_some()
            || profile.profile_version.is_some()
            || profile.profile_family.is_some()
            || profile.profile_sha256.is_some())
    {
        return Err(invalid_schema(
            "status",
            "draft profile must not set frozen-only identity fields",
        ));
    }

    if let Some(hashing) = profile.hashing
        && !matches!(hashing.algorithm, HashAlgorithm::Sha256)
    {
        return Err(invalid_schema("hashing.algorithm", "must be sha256"));
    }

    Ok(())
}

fn validate_pre_parse(profile: &Profile) -> Result<(), RefusalPayload> {
    let Some(pre_parse) = profile.pre_parse.as_ref() else {
        return Ok(());
    };
    let slice = &pre_parse.slice;

    if let Some(encoding) = slice.encoding.as_deref()
        && !encoding.eq_ignore_ascii_case("utf-8")
        && !encoding.eq_ignore_ascii_case("utf8")
    {
        return Err(invalid_schema(
            "pre_parse.slice.encoding",
            "only utf-8 is currently supported",
        ));
    }

    if let Some(delimiter) = slice.delimiter.as_deref()
        && delimiter.chars().count() != 1
    {
        return Err(invalid_schema(
            "pre_parse.slice.delimiter",
            "delimiter must be exactly one character",
        ));
    }

    if let Some(skip_rows) = slice.skip_rows
        && skip_rows == 0
    {
        return Err(invalid_schema(
            "pre_parse.slice.skip_rows",
            "must be positive when set",
        ));
    }
    if let Some(header_at_row) = slice.header_at_row
        && header_at_row == 0
    {
        return Err(invalid_schema(
            "pre_parse.slice.header_at_row",
            "must be positive when set",
        ));
    }
    if let Some(data_starts_at) = slice.data_starts_at
        && data_starts_at == 0
    {
        return Err(invalid_schema(
            "pre_parse.slice.data_starts_at",
            "must be positive when set",
        ));
    }
    if slice.header_rows.contains(&0) {
        return Err(invalid_schema(
            "pre_parse.slice.header_rows",
            "rows are 1-indexed and must be positive",
        ));
    }
    if slice.unit_rows.contains(&0) {
        return Err(invalid_schema(
            "pre_parse.slice.unit_rows",
            "rows are 1-indexed and must be positive",
        ));
    }
    if !is_strictly_increasing(&slice.header_rows) {
        return Err(invalid_schema(
            "pre_parse.slice.header_rows",
            "rows must be strictly increasing",
        ));
    }
    if !is_contiguous(&slice.header_rows) {
        return Err(invalid_schema(
            "pre_parse.slice.header_rows",
            "multi-row headers must be contiguous",
        ));
    }
    if !is_strictly_increasing(&slice.unit_rows) {
        return Err(invalid_schema(
            "pre_parse.slice.unit_rows",
            "rows must be strictly increasing",
        ));
    }

    if let Some(header_merge) = slice.header_merge.as_ref() {
        if let Some(separator) = header_merge.separator.as_deref()
            && separator.is_empty()
        {
            return Err(invalid_schema(
                "pre_parse.slice.header_merge.separator",
                "must be non-empty when set",
            ));
        }
        match header_merge.strategy {
            HeaderMergeStrategy::FfillConcat
            | HeaderMergeStrategy::ConcatOnly
            | HeaderMergeStrategy::FirstNonEmpty => {}
        }
    }

    let last_header_row = slice
        .header_rows
        .iter()
        .copied()
        .chain(slice.header_at_row)
        .max();
    if let (Some(data_starts_at), Some(last_header_row)) = (slice.data_starts_at, last_header_row)
        && data_starts_at <= last_header_row
    {
        return Err(invalid_schema(
            "pre_parse.slice.data_starts_at",
            "must be after the header row(s)",
        ));
    }
    if let Some(data_starts_at) = slice.data_starts_at
        && slice.unit_rows.iter().any(|row| *row >= data_starts_at)
    {
        return Err(invalid_schema(
            "pre_parse.slice.unit_rows",
            "unit rows must come before data_starts_at",
        ));
    }

    match slice.mode {
        SliceMode::PreambleSkip => {
            if slice.header_at_row.is_none() && slice.skip_rows.is_none() {
                return Err(invalid_schema(
                    "pre_parse.slice.header_at_row",
                    "preamble_skip requires header_at_row or skip_rows",
                ));
            }
        }
        SliceMode::MultiRowHeader => {
            if slice.header_rows.len() < 2 {
                return Err(invalid_schema(
                    "pre_parse.slice.header_rows",
                    "multi_row_header requires at least two header rows",
                ));
            }
        }
        SliceMode::PreambleWithUnits => {
            if slice.header_at_row.is_none() {
                return Err(invalid_schema(
                    "pre_parse.slice.header_at_row",
                    "preamble_with_units requires header_at_row",
                ));
            }
            if slice.unit_rows.is_empty() {
                return Err(invalid_schema(
                    "pre_parse.slice.unit_rows",
                    "preamble_with_units requires at least one unit row",
                ));
            }
        }
    }

    Ok(())
}

fn is_strictly_increasing(rows: &[usize]) -> bool {
    rows.windows(2).all(|pair| pair[0] < pair[1])
}

fn is_contiguous(rows: &[usize]) -> bool {
    rows.windows(2).all(|pair| pair[0] + 1 == pair[1])
}

pub fn is_valid_profile_family(family: &str) -> bool {
    let mut segments = family.split('.');
    let Some(first) = segments.next() else {
        return false;
    };

    if !is_first_family_segment(first) {
        return false;
    }

    segments.all(is_family_segment)
}

pub fn is_valid_profile_sha256(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };

    hex.len() == 64
        && hex
            .chars()
            .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch))
}

fn is_first_family_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    first.is_ascii_lowercase() && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
}

fn is_family_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    first.is_ascii_lowercase()
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

fn map_yaml_error(error: serde_yaml::Error) -> RefusalPayload {
    let message = error.to_string();

    if let Some(field) = extract_missing_field(&message) {
        return missing_field(field);
    }

    invalid_schema("yaml", format!("invalid profile YAML: {message}"))
}

fn extract_missing_field(message: &str) -> Option<&str> {
    let (_, after_prefix) = message.split_once("missing field `")?;
    let (field, _) = after_prefix.split_once('`')?;
    Some(field)
}

fn missing_field(field: &str) -> RefusalPayload {
    RefusalPayload::missing_field(field)
}

fn invalid_schema(field: impl Into<String>, error: impl Into<String>) -> RefusalPayload {
    RefusalPayload::invalid_schema_single(field, error)
}
