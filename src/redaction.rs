#![forbid(unsafe_code)]

//! Header redaction for `profile slice`, `inspect`, `stats` and `suggest-key`
//! (bd-ds5, the family redaction ruling).
//!
//! The first rows of a messy workbook are often a title banner, a methodology paragraph or an
//! actual data row, so the "header" a command reads can be raw prose or cell values. Without
//! `--explicit` those strings are never printed. Where a command needs to say "this column" it
//! prints a marker:
//!
//! - **Ordinal refs (default).** `redacted:ref:c1`, `c2`, ... numbered in the order the names
//!   first appear in the file (column order), never in name order. Equal text inside ONE output
//!   shares a ref (a banner repeated across merged cells is one ref). No hash of any text is
//!   computed, so nothing can be guessed from a dictionary. Refs are not comparable across
//!   files. Output is byte-identical when the same file is run again.
//! - **Keyed markers (opt-in).** When `PROFILE_REDACTION_KEY` holds a key of at least 16 bytes,
//!   each string becomes `redacted:hmac-sha256:<32 hex>`: the first 128 bits of
//!   HMAC-SHA256(key, `<kind>` NUL `<text>`) with `<kind>` = `column` (the tag is a domain separator
//!   reserved for further kinds). Stable
//!   across files and runs for the key holder (the correlation feature), unguessable without
//!   the key. An empty or too-short key is refused (`E_REDACTION_KEY_INVALID`), never silently
//!   downgraded to ordinal refs.
//!
//! What a key holder CAN learn: that two strings of the same kind in different outputs are
//! equal, and whether a candidate string is present (by computing its marker). Nobody can read
//! the text out of a marker. Counts, types and positions are not redacted.

use std::collections::BTreeMap;
use std::ffi::OsString;

use hmac::{Hmac, Mac};
use sha2::Sha256;

pub const ENV_KEY: &str = "PROFILE_REDACTION_KEY";
pub const MIN_KEY_BYTES: usize = 16;
pub const REF_PREFIX: &str = "redacted:ref:";
pub const HMAC_PREFIX: &str = "redacted:hmac-sha256:";

/// Why `PROFILE_REDACTION_KEY` was refused. Never carries the key or its length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyError {
    Empty,
    TooShort,
}

impl KeyError {
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::TooShort => "too_short",
        }
    }
}

/// An operator-held redaction key. `Debug` never prints the bytes.
#[derive(Clone)]
pub struct RedactionKey(Vec<u8>);

impl std::fmt::Debug for RedactionKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RedactionKey(<withheld>)")
    }
}

impl RedactionKey {
    /// `None` when the variable is unset (ordinal refs). Set-but-empty and too-short values are
    /// errors, so a misconfigured key cannot quietly fall back.
    pub fn from_env_value(value: Option<OsString>) -> Result<Option<Self>, KeyError> {
        let Some(value) = value else {
            return Ok(None);
        };
        #[cfg(unix)]
        let bytes = std::os::unix::ffi::OsStringExt::into_vec(value);
        #[cfg(not(unix))]
        let bytes = value.to_string_lossy().into_owned().into_bytes();
        if bytes.is_empty() {
            return Err(KeyError::Empty);
        }
        if bytes.len() < MIN_KEY_BYTES {
            return Err(KeyError::TooShort);
        }
        Ok(Some(Self(bytes)))
    }

    pub fn from_env() -> Result<Option<Self>, KeyError> {
        Self::from_env_value(std::env::var_os(ENV_KEY))
    }

    /// `redacted:hmac-sha256:<32 hex>` over `kind NUL text`.
    pub fn marker(&self, kind: Kind, text: &str) -> String {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&self.0).expect("HMAC accepts a key of any length");
        mac.update(kind.tag().as_bytes());
        mac.update(&[0]);
        mac.update(text.as_bytes());
        let digest = mac.finalize().into_bytes();
        let mut hex = String::with_capacity(32);
        for byte in &digest[..16] {
            hex.push_str(&format!("{byte:02x}"));
        }
        format!("{HMAC_PREFIX}{hex}")
    }
}

/// What kind of text is being marked: each kind is its own ordinal numbering and HMAC domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Column,
}

impl Kind {
    /// HMAC domain separator.
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Column => "column",
        }
    }

    const fn letter(self) -> char {
        match self {
            Self::Column => 'c',
        }
    }
}

/// Turns strings into markers for ONE output. Mark strings in the order they appear in the file.
pub struct Marker {
    key: Option<RedactionKey>,
    seen: BTreeMap<(Kind, String), usize>,
    counters: BTreeMap<Kind, usize>,
}

impl Marker {
    pub fn new(key: Option<RedactionKey>) -> Self {
        Self {
            key,
            seen: BTreeMap::new(),
            counters: BTreeMap::new(),
        }
    }

    /// A marker keyed from `PROFILE_REDACTION_KEY`; a set-but-unusable key is a typed refusal
    /// (never a fallback to ordinal refs).
    pub fn from_env() -> Result<Self, crate::refusal::RefusalPayload> {
        RedactionKey::from_env()
            .map(Self::new)
            .map_err(crate::refusal::RefusalPayload::redaction_key_invalid)
    }

    /// The marker for one command output: `--explicit` prints names and needs none, otherwise the
    /// key comes from the environment (an unusable key is a typed refusal).
    pub fn for_output(explicit: bool) -> Result<Self, crate::refusal::RefusalPayload> {
        if explicit {
            Ok(Self::new(None))
        } else {
            Self::from_env()
        }
    }

    /// `"keyed"` or `"ordinal"`: recorded in the output (never the key).
    pub fn mode(&self) -> &'static str {
        if self.key.is_some() {
            "keyed"
        } else {
            "ordinal"
        }
    }

    pub fn mark(&mut self, kind: Kind, text: &str) -> String {
        if let Some(key) = &self.key {
            return key.marker(kind, text);
        }
        let next = self.counters.entry(kind).or_insert(0);
        let ordinal = *self.seen.entry((kind, text.to_owned())).or_insert_with(|| {
            *next += 1;
            *next
        });
        format!("{REF_PREFIX}{}{ordinal}", kind.letter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinals_follow_first_appearance_not_text_order_and_equal_text_shares() {
        let mut marker = Marker::new(None);
        assert_eq!(marker.mark(Kind::Column, "zebra"), "redacted:ref:c1");
        assert_eq!(marker.mark(Kind::Column, "apple"), "redacted:ref:c2");
        assert_eq!(marker.mark(Kind::Column, "zebra"), "redacted:ref:c1");
    }

    #[test]
    fn keys_are_validated_without_leaking_length_or_value() {
        let make = |v: &str| RedactionKey::from_env_value(Some(OsString::from(v)));
        assert_eq!(make("").unwrap_err(), KeyError::Empty);
        assert_eq!(make("0123456789abcde").unwrap_err(), KeyError::TooShort);
        assert!(make("0123456789abcdef").unwrap().is_some());
        assert!(RedactionKey::from_env_value(None).unwrap().is_none());
        let key = make("0123456789abcdef").unwrap().unwrap();
        assert!(!format!("{key:?}").contains("0123456789"));
    }

    #[test]
    fn keyed_markers_separate_kinds_and_change_with_the_key() {
        let a = RedactionKey::from_env_value(Some(OsString::from("0123456789abcdef")))
            .unwrap()
            .unwrap();
        let b = RedactionKey::from_env_value(Some(OsString::from("fedcba9876543210")))
            .unwrap()
            .unwrap();
        assert_ne!(a.marker(Kind::Column, "x"), b.marker(Kind::Column, "x"));
        assert_eq!(a.marker(Kind::Column, "x"), a.marker(Kind::Column, "x"));
        // Independent openssl known answer:
        // printf 'column\0loan_id' | openssl dgst -sha256 -hmac 0123456789abcdef | cut -c1-32
        assert_eq!(
            a.marker(Kind::Column, "loan_id"),
            "redacted:hmac-sha256:9f6181c104894742edc83d76f18dc14f"
        );
    }
}
