//! Refusal system and error codes
//!
//! This module implements the complete refusal system for the profile CLI,
//! including all 8 refusal codes and their structured detail payloads.

pub mod codes;
pub mod payload;

// Re-export key types
pub use codes::RefusalCode;
pub use payload::{
    AlreadyFrozenDetail, BadVersionDetail, ColumnNotFoundDetail, CsvParseDetail, EmptyDetail,
    FieldError, InvalidSchemaDetail, IoDetail, MissingFieldDetail, RefusalPayload,
};

// All exports are handled above
