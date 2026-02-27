use serde_json::{Value, json};

use crate::cli::args::DiffArgs;
use crate::refusal::RefusalPayload;
use crate::resolve::resolver::resolve_profile;
use crate::schema::profile::Profile;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProfileDifference {
    pub field: String,
    pub a_value: Value,
    pub b_value: Value,
}

pub fn handle(a: &str, b: &str) -> Result<Value, RefusalPayload> {
    // Resolve both profiles
    let resolved_a = resolve_profile(a)?;
    let resolved_b = resolve_profile(b)?;

    // Compare semantic fields and collect differences
    let differences = compare_profiles(&resolved_a.profile, &resolved_b.profile);

    // Return result with differences
    Ok(json!({
        "a_path": resolved_a.path.display().to_string(),
        "b_path": resolved_b.path.display().to_string(),
        "differences": differences,
        "equivalent": differences.is_empty()
    }))
}

pub fn run(args: &DiffArgs, _no_witness: bool) -> Result<Value, RefusalPayload> {
    handle(&args.a, &args.b)
}

fn compare_profiles(a: &Profile, b: &Profile) -> Vec<ProfileDifference> {
    let mut differences = Vec::new();

    // Compare format
    if a.format != b.format {
        differences.push(ProfileDifference {
            field: "format".to_string(),
            a_value: json!(a.format),
            b_value: json!(b.format),
        });
    }

    // Compare hashing
    if a.hashing != b.hashing {
        differences.push(ProfileDifference {
            field: "hashing".to_string(),
            a_value: json!(a.hashing),
            b_value: json!(b.hashing),
        });
    }

    // Compare equivalence
    if a.equivalence != b.equivalence {
        differences.push(ProfileDifference {
            field: "equivalence".to_string(),
            a_value: json!(a.equivalence),
            b_value: json!(b.equivalence),
        });
    }

    // Compare key (order-sensitive)
    if a.key != b.key {
        differences.push(ProfileDifference {
            field: "key".to_string(),
            a_value: json!(a.key),
            b_value: json!(b.key),
        });
    }

    // Compare include_columns (order-sensitive)
    if a.include_columns != b.include_columns {
        differences.push(ProfileDifference {
            field: "include_columns".to_string(),
            a_value: json!(a.include_columns),
            b_value: json!(b.include_columns),
        });
    }

    differences
}
