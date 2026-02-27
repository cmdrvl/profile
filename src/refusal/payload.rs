use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::refusal::codes::RefusalCode;

/// Complete refusal payload with code, message, and typed detail
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefusalPayload {
    /// The refusal code (e.g., "E_INVALID_SCHEMA")
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// Structured detail payload specific to the refusal code
    pub detail: Value,
    /// Optional suggested remediation command
    pub next_command: Option<String>,
}

/// Schema validation error detail
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvalidSchemaDetail {
    pub errors: Vec<FieldError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldError {
    pub field: String,
    pub error: String,
}

/// Missing field error detail
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingFieldDetail {
    pub field: String,
}

/// Bad version error detail
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BadVersionDetail {
    pub family: String,
    pub version: u64,
    pub error: String,
}

/// Already frozen error detail
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlreadyFrozenDetail {
    pub profile_id: String,
    pub profile_sha256: String,
}

/// IO error detail
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IoDetail {
    pub path: String,
    pub error: String,
}

/// CSV parse error detail
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsvParseDetail {
    pub path: String,
    pub error: String,
}

/// Empty dataset error detail
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmptyDetail {
    pub path: String,
    pub reason: String,
}

/// Column not found error detail
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnNotFoundDetail {
    pub columns: Vec<String>,
    pub available: Vec<String>,
}

impl RefusalPayload {
    /// Create a new refusal payload with the given code and detail
    pub fn new(code: RefusalCode, detail: impl Serialize) -> Self {
        let detail = serde_json::to_value(detail).expect("detail must be serializable");

        Self {
            code: code.as_str().to_string(),
            message: code.message().to_string(),
            detail,
            next_command: None,
        }
    }

    /// Create an invalid schema refusal
    pub fn invalid_schema(errors: Vec<FieldError>) -> Self {
        Self::new(RefusalCode::InvalidSchema, InvalidSchemaDetail { errors })
    }

    /// Create a single field error for invalid schema
    pub fn invalid_schema_single(field: impl Into<String>, error: impl Into<String>) -> Self {
        Self::invalid_schema(vec![FieldError {
            field: field.into(),
            error: error.into(),
        }])
    }

    /// Create a missing field refusal
    pub fn missing_field(field: impl Into<String>) -> Self {
        Self::new(
            RefusalCode::MissingField,
            MissingFieldDetail {
                field: field.into(),
            },
        )
    }

    /// Create a bad version refusal
    pub fn bad_version(family: impl Into<String>, version: u64, error: impl Into<String>) -> Self {
        Self::new(
            RefusalCode::BadVersion,
            BadVersionDetail {
                family: family.into(),
                version,
                error: error.into(),
            },
        )
    }

    /// Create an already frozen refusal
    pub fn already_frozen(
        profile_id: impl Into<String>,
        profile_sha256: impl Into<String>,
    ) -> Self {
        Self::new(
            RefusalCode::AlreadyFrozen,
            AlreadyFrozenDetail {
                profile_id: profile_id.into(),
                profile_sha256: profile_sha256.into(),
            },
        )
    }

    /// Create an IO refusal
    pub fn io(path: impl Into<String>, error: impl Into<String>) -> Self {
        Self::new(
            RefusalCode::Io,
            IoDetail {
                path: path.into(),
                error: error.into(),
            },
        )
    }

    /// Create a CSV parse refusal
    pub fn csv_parse(path: impl Into<String>, error: impl Into<String>) -> Self {
        Self::new(
            RefusalCode::CsvParse,
            CsvParseDetail {
                path: path.into(),
                error: error.into(),
            },
        )
    }

    /// Create an empty dataset refusal
    pub fn empty(path: impl Into<String>) -> Self {
        Self::empty_with_reason(path, "no data rows")
    }

    /// Create an empty dataset refusal with explicit reason
    pub fn empty_with_reason(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(
            RefusalCode::Empty,
            EmptyDetail {
                path: path.into(),
                reason: reason.into(),
            },
        )
    }

    /// Create a column not found refusal
    pub fn column_not_found(columns: Vec<String>, available: Vec<String>) -> Self {
        Self::new(
            RefusalCode::ColumnNotFound,
            ColumnNotFoundDetail { columns, available },
        )
    }
}

impl std::fmt::Display for RefusalPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for RefusalPayload {}

// Legacy compatibility - simple RefusalPayload constructor for existing code
impl RefusalPayload {
    /// Create a simple refusal payload (legacy compatibility)
    pub fn simple(code: impl Into<String>, detail: impl Into<String>) -> Self {
        let detail_str = detail.into();
        Self {
            code: code.into(),
            message: detail_str.clone(),
            detail: serde_json::Value::String(detail_str),
            next_command: None,
        }
    }

    /// Attach a suggested next command to this refusal.
    pub fn with_next_command(mut self, next_command: impl Into<String>) -> Self {
        self.next_command = Some(next_command.into());
        self
    }
}
