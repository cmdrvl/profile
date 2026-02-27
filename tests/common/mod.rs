use assert_cmd::{assert::Assert, Command};
use predicates::prelude::predicate;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_ISSUES_FOUND: i32 = 1;
pub const EXIT_REFUSAL: i32 = 2;

pub fn profile_cmd() -> Command {
    Command::cargo_bin("profile").expect("profile binary should be available for integration tests")
}

pub fn fixture_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(relative)
}

pub fn temp_workspace() -> TempDir {
    tempdir().expect("temporary directory should be created")
}

pub fn copy_fixture(relative: &str, destination: impl AsRef<Path>) -> PathBuf {
    let source = fixture_path(relative);
    let destination = destination.as_ref().to_path_buf();

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("destination parent directory should be created");
    }

    fs::copy(&source, &destination).expect("fixture copy should succeed");
    destination
}

pub fn parse_stdout_json(assert: &Assert) -> Value {
    serde_json::from_slice(&assert.get_output().stdout).expect("stdout should contain valid JSON")
}

pub fn assert_json_envelope_shape(value: &Value) {
    for key in [
        "version",
        "outcome",
        "exit_code",
        "subcommand",
        "result",
        "profile_ref",
        "witness_id",
    ] {
        assert!(
            value.get(key).is_some(),
            "expected output envelope to include key: {key}"
        );
    }
}

pub fn assert_stdout_contains(assert: Assert, needle: &str) -> Assert {
    assert.stdout(predicate::str::contains(needle))
}

macro_rules! assert_exit_code {
    ($assert:expr, $code:expr) => {{
        $assert.code($code);
    }};
}
pub(crate) use assert_exit_code;

macro_rules! assert_success_exit {
    ($assert:expr) => {{
        $assert.code($crate::common::EXIT_SUCCESS);
    }};
}
pub(crate) use assert_success_exit;

macro_rules! assert_issues_exit {
    ($assert:expr) => {{
        $assert.code($crate::common::EXIT_ISSUES_FOUND);
    }};
}
pub(crate) use assert_issues_exit;

macro_rules! assert_refusal_exit {
    ($assert:expr) => {{
        $assert.code($crate::common::EXIT_REFUSAL);
    }};
}
pub(crate) use assert_refusal_exit;
