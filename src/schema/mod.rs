pub mod canonical;
pub mod profile;
pub mod registry;
pub mod validate;

pub use canonical::{canonical_bytes, canonical_yaml, compute_profile_sha256};
pub use profile::{
    Equivalence, EquivalenceOrder, HashAlgorithm, Hashing, Profile, ProfileFormat, ProfileStatus,
};
pub use registry::{
    HeaderIndex, build_header_index, canonicalize_header_sequence, canonicalize_profile_column,
    load_column_registry_aliases, resolve_registry_path,
};
pub use validate::{
    ValidationMode, is_valid_profile_family, is_valid_profile_sha256, parse_profile_yaml,
    validate_profile,
};
