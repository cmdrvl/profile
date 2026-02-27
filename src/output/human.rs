use serde_json::Value;

use crate::cli::exit::{EXIT_ISSUES_FOUND, EXIT_REFUSAL, EXIT_SUCCESS};
use crate::refusal::RefusalPayload;

/// Emit human-readable output without JSON envelope
pub fn emit(subcommand: &str, result: Result<Value, RefusalPayload>) -> u8 {
    match result {
        Ok(value) => {
            emit_human_value(subcommand, &value);
            if is_issues_found(subcommand, &value) {
                EXIT_ISSUES_FOUND
            } else {
                EXIT_SUCCESS
            }
        }
        Err(refusal) => {
            emit_human_refusal(&refusal);
            EXIT_REFUSAL
        }
    }
}

fn emit_human_value(subcommand: &str, value: &Value) {
    match subcommand {
        "validate" => emit_validate_result(value),
        "stats" => emit_stats_result(value),
        "suggest-key" => emit_suggest_key_result(value),
        "freeze" => emit_freeze_result(value),
        "list" => emit_list_result(value),
        "show" => emit_show_result(value),
        "diff" => emit_diff_result(value),
        "push" => emit_push_result(value),
        "pull" => emit_pull_result(value),
        "describe" => emit_describe_result(value),
        "schema" => emit_schema_result(value),
        _ => {
            // Fallback: pretty print the JSON value
            match serde_json::to_string_pretty(value) {
                Ok(pretty) => println!("{}", pretty),
                Err(_) => println!("{}", value),
            }
        }
    }
}

fn emit_human_refusal(refusal: &RefusalPayload) {
    eprintln!("Error: {}", refusal.message);

    // Show structured detail in human-readable format
    if let Ok(detail_str) = serde_json::to_string_pretty(&refusal.detail)
        && detail_str != "null"
        && detail_str != "\"\""
    {
        eprintln!("\nDetails:");
        eprintln!("{}", detail_str);
    }

    if let Some(next_command) = &refusal.next_command {
        eprintln!("\nSuggested action: {}", next_command);
    }
}

// Subcommand-specific human output formatters
fn emit_validate_result(value: &Value) {
    if let Some(obj) = value.as_object() {
        if let Some(valid) = obj.get("valid").and_then(|v| v.as_bool()) {
            if valid {
                println!("✓ Profile is valid");
            } else {
                println!("✗ Profile validation failed");
                if let Some(errors) = obj.get("errors").and_then(|v| v.as_array()) {
                    for error in errors {
                        if let Some(err_str) = error.as_str() {
                            println!("  - {}", err_str);
                        }
                    }
                }
            }
        }
    } else {
        // Fallback
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_default()
        );
    }
}

fn emit_stats_result(value: &Value) {
    if let Some(obj) = value.as_object() {
        println!("Profile Statistics:");
        if let Some(columns) = obj.get("columns").and_then(|v| v.as_u64()) {
            println!("  Total columns: {}", columns);
        }
        if let Some(key_columns) = obj.get("key_columns").and_then(|v| v.as_u64()) {
            println!("  Key columns: {}", key_columns);
        }
        if let Some(include_columns) = obj.get("include_columns").and_then(|v| v.as_u64()) {
            println!("  Include columns: {}", include_columns);
        }
    } else {
        // Fallback
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_default()
        );
    }
}

fn emit_suggest_key_result(value: &Value) {
    if let Some(obj) = value.as_object() {
        if let Some(candidates) = obj.get("candidates").and_then(|v| v.as_array()) {
            if candidates.is_empty() {
                println!("No key column candidates found");
            } else {
                println!("Key column suggestions:");
                for candidate in candidates {
                    if let Some(col) = candidate.get("column").and_then(|v| v.as_str()) {
                        let uniqueness = candidate
                            .get("uniqueness")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);
                        let viable = candidate
                            .get("viable")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let rank = candidate.get("rank").and_then(|v| v.as_u64()).unwrap_or(0);

                        let status = if viable { "✓" } else { "✗" };
                        println!(
                            "  {}. {} {} (uniqueness: {:.2})",
                            rank, status, col, uniqueness
                        );
                    }
                }
            }
        }
    } else {
        // Fallback
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_default()
        );
    }
}

fn emit_freeze_result(value: &Value) {
    if let Some(obj) = value.as_object() {
        if let Some(profile_id) = obj.get("profile_id").and_then(|v| v.as_str()) {
            println!("✓ Profile frozen successfully");
            println!("  Profile ID: {}", profile_id);

            if let Some(sha256) = obj.get("profile_sha256").and_then(|v| v.as_str()) {
                println!("  SHA256: {}", sha256);
            }
        }
    } else {
        // Fallback
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_default()
        );
    }
}

fn emit_list_result(value: &Value) {
    if let Some(obj) = value.as_object() {
        if let Some(profiles) = obj.get("profiles").and_then(|v| v.as_array()) {
            if profiles.is_empty() {
                println!("No profiles found");
            } else {
                println!("Found {} profile(s):", profiles.len());
                for profile in profiles {
                    if let Some(path) = profile.get("path").and_then(|v| v.as_str()) {
                        print!("  {}", path);
                        if let Some(id) = profile.get("profile_id").and_then(|v| v.as_str()) {
                            print!(" ({})", id);
                        }
                        println!();
                    }
                }
            }
        }
    } else {
        // Fallback
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_default()
        );
    }
}

fn emit_show_result(value: &Value) {
    if let Some(obj) = value.as_object() {
        if let Some(profile) = obj.get("profile") {
            println!("Profile Details:");
            println!(
                "{}",
                serde_json::to_string_pretty(profile).unwrap_or_default()
            );
        }
    } else {
        // Fallback
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_default()
        );
    }
}

fn emit_diff_result(value: &Value) {
    if let Some(obj) = value.as_object() {
        let differences = obj
            .get("differences")
            .or_else(|| obj.get("changes"))
            .and_then(|v| v.as_array());

        if let Some(diffs) = differences {
            if diffs.is_empty() {
                println!("✓ Profiles are equivalent");
            } else {
                println!("✗ {} difference(s) found:", diffs.len());
                for diff in diffs {
                    if let Some(diff_obj) = diff.as_object() {
                        let field = diff_obj
                            .get("field")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let a_value = &diff_obj.get("a_value").unwrap_or(&serde_json::Value::Null);
                        let b_value = &diff_obj.get("b_value").unwrap_or(&serde_json::Value::Null);

                        println!("  Field '{}' differs:", field);
                        println!("    A: {}", format_value_compact(a_value));
                        println!("    B: {}", format_value_compact(b_value));
                    }
                }
            }
        } else {
            // Check for equivalent field (boolean)
            if let Some(equivalent) = obj.get("equivalent").and_then(|v| v.as_bool()) {
                if equivalent {
                    println!("✓ Profiles are equivalent");
                } else {
                    println!("✗ Profiles differ");
                }
            } else {
                // Fallback
                println!(
                    "{}",
                    serde_json::to_string_pretty(value).unwrap_or_default()
                );
            }
        }
    } else {
        // Fallback
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_default()
        );
    }
}

fn format_value_compact(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("\"{}\"", s),
        Value::Array(arr) => {
            let items = arr
                .iter()
                .map(format_value_compact)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{}]", items)
        }
        Value::Object(_) => serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string()),
    }
}

fn emit_describe_result(value: &Value) {
    if let Some(obj) = value.as_object() {
        if let Some(description) = obj.get("description").and_then(|v| v.as_str()) {
            println!("{}", description);
        } else {
            // Show key profile information
            if let Some(profile_id) = obj.get("profile_id").and_then(|v| v.as_str()) {
                println!("Profile: {}", profile_id);
            }
            if let Some(status) = obj.get("status").and_then(|v| v.as_str()) {
                println!("Status: {}", status);
            }
        }
    } else {
        // Fallback
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_default()
        );
    }
}

fn emit_schema_result(value: &Value) {
    // Schema output is typically just the JSON schema, so pretty print it
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

fn emit_push_result(value: &Value) {
    if let Some(obj) = value.as_object()
        && let Some(profile_id) = obj.get("profile_id").and_then(|v| v.as_str())
    {
        println!("✓ Published profile: {}", profile_id);
        if let Some(profile_sha) = obj.get("profile_sha256").and_then(|v| v.as_str()) {
            println!("  SHA256: {}", profile_sha);
        }
        return;
    }

    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

fn emit_pull_result(value: &Value) {
    if let Some(obj) = value.as_object()
        && let Some(profile_id) = obj.get("profile_id").and_then(|v| v.as_str())
    {
        println!("✓ Pulled profile: {}", profile_id);
        if let Some(path) = obj.get("path").and_then(|v| v.as_str()) {
            println!("  Path: {}", path);
        }
        return;
    }

    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

fn is_issues_found(subcommand: &str, value: &Value) -> bool {
    match subcommand {
        "lint" => value
            .get("issues")
            .and_then(Value::as_array)
            .is_some_and(|issues| !issues.is_empty()),
        "diff" => value
            .get("differences")
            .or_else(|| value.get("changes"))
            .and_then(Value::as_array)
            .is_some_and(|changes| !changes.is_empty()),
        _ => false,
    }
}
