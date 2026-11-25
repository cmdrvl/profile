//! Post-processing of a refusal after a command fails (bd-ds5): the real subcommand in
//! `next_command` with a shell-quoted path, and no dataset text in the detail unless `--explicit`.

use std::path::Path;

use serde_json::{Value, json};

use super::RefusalPayload;
use crate::cli::args::{Command, DraftCommand};

/// POSIX shell quoting: a word made only of safe characters stays as is, anything else
/// (spaces, parentheses, quotes, `$`, ...) is wrapped in single quotes with embedded single
/// quotes written as `'\''`.
pub fn shell_quote(text: &str) -> String {
    let safe = !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-./:@%+=,".contains(c));
    if safe {
        text.to_owned()
    } else {
        format!("'{}'", text.replace('\'', "'\\''"))
    }
}

fn quote_path(path: &Path) -> String {
    shell_quote(&path.display().to_string())
}

/// A copy-pasteable command that reruns `command` on a chosen worksheet: the real subcommand
/// name and its own required arguments, paths quoted. `None` for commands that read no dataset.
pub fn sheet_next_command(command: &Command) -> Option<String> {
    let line = match command {
        Command::Slice(args) => format!("profile slice {}", quote_path(&args.file)),
        Command::Inspect(args) => format!("profile inspect {}", quote_path(&args.dataset)),
        Command::Stats(args) => format!("profile stats {}", quote_path(&args.dataset)),
        Command::SuggestKey(args) => format!("profile suggest-key {}", quote_path(&args.dataset)),
        Command::Lint(args) => format!(
            "profile lint {} --against {}",
            quote_path(&args.profile),
            quote_path(&args.against)
        ),
        Command::Evolve(args) => format!(
            "profile evolve {} --against {} --out {}",
            quote_path(&args.profile),
            quote_path(&args.against),
            quote_path(&args.out)
        ),
        Command::Draft(draft) => match &draft.command {
            DraftCommand::Init(args) => format!(
                "profile draft init {} --out {}",
                quote_path(&args.dataset),
                quote_path(&args.out)
            ),
            DraftCommand::New(_) => return None,
        },
        _ => return None,
    };
    Some(format!(
        "{line} --sheet 0   # keep your other flags; --sheet takes a sheet index (from 0) or name"
    ))
}

/// Apply the finishing rules to a refusal raised by `command`.
pub fn finish(command: &Command, explicit: bool, mut payload: RefusalPayload) -> RefusalPayload {
    let tabular_code = payload
        .detail
        .get("tabular_code")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if payload.code == "E_INPUT_FORMAT"
        && matches!(
            tabular_code.as_deref(),
            Some("E_AMBIGUOUS_SHEET" | "E_SHEET_NOT_FOUND")
        )
    {
        if let Some(next) = sheet_next_command(command) {
            payload.next_command = Some(next);
        }
        if !explicit
            && let Some(context) = payload
                .detail
                .get_mut("context")
                .and_then(Value::as_object_mut)
            && let Some(names) = context
                .remove("sheets")
                .or_else(|| context.remove("sheet_names"))
        {
            // Sheet names are workbook content: the count says which indexes exist.
            context.insert(
                "sheet_count".to_owned(),
                json!(names.as_array().map_or(0, Vec::len)),
            );
        }
    }
    if payload.code == "E_COLUMN_NOT_FOUND"
        && !explicit
        && let Some(detail) = payload.detail.as_object_mut()
        && let Some(available) = detail.remove("available")
    {
        // The dataset's header text is withheld; `columns` are the names the caller asked for.
        detail.insert(
            "available_count".to_owned(),
            json!(available.as_array().map_or(0, Vec::len)),
        );
    }
    payload
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_keeps_safe_words_and_wraps_the_rest() {
        assert_eq!(shell_quote("data/report.xlsx"), "data/report.xlsx");
        assert_eq!(shell_quote("report (1).xlsx"), "'report (1).xlsx'");
        assert_eq!(shell_quote("it's.xlsx"), "'it'\\''s.xlsx'");
        assert_eq!(shell_quote("a$b;c.xlsx"), "'a$b;c.xlsx'");
        assert_eq!(shell_quote(""), "''");
    }
}
