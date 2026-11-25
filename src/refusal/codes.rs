/// Refusal codes for the profile CLI
///
/// Each code represents a specific type of error that prevents the operation
/// from completing successfully. All operations exit with code 2 (REFUSAL)
/// when these errors occur.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusalCode {
    /// Profile fails schema validation
    InvalidSchema,
    /// Required field not declared
    MissingField,
    /// Family/version syntax or integer constraints failed
    BadVersion,
    /// Profile already frozen (attempted to freeze again)
    AlreadyFrozen,
    /// Can't read/write file
    Io,
    /// Can't parse dataset
    CsvParse,
    /// Dataset missing header or data rows required for operation
    Empty,
    /// Column not found in dataset
    ColumnNotFound,
    /// TSV / JSONL / Parquet / XLSX input refused by the shared tabular reader
    InputFormat,
    /// PROFILE_REDACTION_KEY is set but empty or shorter than 16 bytes
    RedactionKeyInvalid,
    /// Human-mode `slice` without `--out` or `--explicit`: raw CSV rows never reach stdout by default
    OutputRequired,
}

impl RefusalCode {
    /// Returns the string code used in JSON output and error messages
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidSchema => "E_INVALID_SCHEMA",
            Self::MissingField => "E_MISSING_FIELD",
            Self::BadVersion => "E_BAD_VERSION",
            Self::AlreadyFrozen => "E_ALREADY_FROZEN",
            Self::Io => "E_IO",
            Self::CsvParse => "E_CSV_PARSE",
            Self::Empty => "E_EMPTY",
            Self::ColumnNotFound => "E_COLUMN_NOT_FOUND",
            Self::InputFormat => "E_INPUT_FORMAT",
            Self::RedactionKeyInvalid => "E_REDACTION_KEY_INVALID",
            Self::OutputRequired => "E_OUTPUT_REQUIRED",
        }
    }

    /// Returns the human-readable message for this refusal code
    pub const fn message(&self) -> &'static str {
        match self {
            Self::InvalidSchema => "Profile fails schema validation",
            Self::MissingField => "Required field not declared",
            Self::BadVersion => "Family/version syntax or integer constraints failed",
            Self::AlreadyFrozen => "Profile already frozen",
            Self::Io => "Can't read/write file",
            Self::CsvParse => "Can't parse dataset",
            Self::Empty => "Dataset missing header or data rows required for operation",
            Self::ColumnNotFound => "Column not found in dataset",
            Self::InputFormat => {
                "Input refused by the shared tabular reader (TSV, JSONL, Parquet, XLSX)"
            }
            Self::RedactionKeyInvalid => "PROFILE_REDACTION_KEY is empty or too short",
            Self::OutputRequired => {
                "slice would print the CSV to stdout; pass --out <path> or --explicit"
            }
        }
    }

    /// Returns the suggested action for this refusal code
    pub const fn action(&self) -> &'static str {
        match self {
            Self::InvalidSchema
            | Self::MissingField
            | Self::BadVersion
            | Self::CsvParse
            | Self::Empty
            | Self::ColumnNotFound
            | Self::InputFormat
            | Self::RedactionKeyInvalid
            | Self::OutputRequired => "fix_input",
            Self::AlreadyFrozen | Self::Io => "escalate",
        }
    }
}

impl std::fmt::Display for RefusalCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
