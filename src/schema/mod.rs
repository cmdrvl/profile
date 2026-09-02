pub mod canonical;
pub mod profile;
pub mod registry;
pub mod validate;

pub use canonical::{canonical_bytes, canonical_yaml, compute_profile_sha256};
pub use profile::{
    Equivalence, EquivalenceOrder, ExpectedShape, HashAlgorithm, Hashing, HeaderMerge,
    HeaderMergeStrategy, PreParse, Profile, ProfileFormat, ProfileStatus, SliceDirectives,
    SliceMode,
};
pub use registry::{
    HeaderIndex, build_header_index, canonicalize_header_sequence, canonicalize_profile_column,
    load_column_registry_aliases, registry_content_hash, resolve_registry_path,
    validate_frozen_registry_hash,
};
pub use validate::{
    ValidationMode, is_supported_slice_encoding, is_valid_column_registry_hash,
    is_valid_profile_family, is_valid_profile_sha256, parse_profile_yaml, validate_profile,
};
