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
    pub hashing: Option<Hashing>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equivalence: Option<Equivalence>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key: Vec<String>,

    pub include_columns: Vec<String>,
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
