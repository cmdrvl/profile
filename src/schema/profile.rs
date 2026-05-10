use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProfileStatus {
    Draft,
    Frozen,
}

impl ProfileStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Frozen => "frozen",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProfileFormat {
    Csv,
}

impl ProfileFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Csv => "csv",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum HashAlgorithm {
    #[default]
    Sha256,
}

impl HashAlgorithm {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sha256 => "sha256",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Hashing {
    pub algorithm: HashAlgorithm,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EquivalenceOrder {
    OrderInvariant,
    OrderSensitive,
}

impl EquivalenceOrder {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OrderInvariant => "order-invariant",
            Self::OrderSensitive => "order-sensitive",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct Equivalence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<EquivalenceOrder>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub float_decimals: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim_strings: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema_version: u32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_version: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_family: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_sha256: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen: Option<bool>,

    pub status: ProfileStatus,
    pub format: ProfileFormat,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column_registry: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint_ref: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_parse: Option<PreParse>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hashing: Option<Hashing>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equivalence: Option<Equivalence>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key: Vec<String>,

    pub include_columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PreParse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_shape: Option<ExpectedShape>,
    pub slice: SliceDirectives,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ExpectedShape {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modal_column_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_data_row: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub header_rows_pattern: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SliceDirectives {
    pub mode: SliceMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_rows: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_at_row: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub header_rows: Vec<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_merge: Option<HeaderMerge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_starts_at: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delimiter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preamble_capture: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_rows_capture: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unit_rows: Vec<usize>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SliceMode {
    PreambleSkip,
    MultiRowHeader,
    PreambleWithUnits,
}

impl SliceMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreambleSkip => "preamble_skip",
            Self::MultiRowHeader => "multi_row_header",
            Self::PreambleWithUnits => "preamble_with_units",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HeaderMerge {
    pub strategy: HeaderMergeStrategy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub separator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty_placeholder: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HeaderMergeStrategy {
    FfillConcat,
    ConcatOnly,
    FirstNonEmpty,
}

impl HeaderMergeStrategy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FfillConcat => "ffill_concat",
            Self::ConcatOnly => "concat_only",
            Self::FirstNonEmpty => "first_non_empty",
        }
    }
}

impl Profile {
    pub fn from_yaml(content: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(content)
    }

    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    pub fn is_frozen(&self) -> bool {
        matches!(self.status, ProfileStatus::Frozen)
    }

    pub fn fill_freeze_defaults(&mut self) {
        if self.hashing.is_none() {
            self.hashing = Some(Hashing::default());
        }

        let equivalence = self.equivalence.get_or_insert_with(Equivalence::default);
        if equivalence.order.is_none() {
            equivalence.order = Some(EquivalenceOrder::OrderInvariant);
        }
    }
}
