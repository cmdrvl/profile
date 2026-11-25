//! Non-CSV datasets (JSONL, Parquet, XLSX/XLS) via the shared `cmdrvl-tabular` reader.
//!
//! Every profile command that reads a dataset (`draft init`, `suggest-key`, `lint`, `stats`,
//! `slice`) is built around a CSV reader. A non-CSV dataset is read through tabular and
//! re-rendered as canonical CSV bytes in memory (comma, `\n` line ends, quotes only where
//! needed), then handed to the unchanged CSV code. Consequences:
//!
//! - identical data in any container gives identical bytes here, so every command reports the
//!   same thing across containers (golden tests in `tests/formats.rs`);
//! - `.csv`, `.txt` and extensionless files never come through here, so existing
//!   output and the hashes derived from it cannot change;
//! - nothing is written to disk and cell values never appear in a refusal.

use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;

use cmdrvl_tabular::{Format, OpenOptions, RefusalCode as TabularCode, SheetSel, open};

use crate::refusal::RefusalPayload;

/// Container formats that go through tabular, by extension.
pub fn is_tabular_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("jsonl" | "ndjson" | "parquet" | "xlsx" | "xlsm" | "xls" | "tsv")
    )
}

/// Read `path` through tabular and render it as CSV bytes.
pub fn render_csv(path: &Path, sheet: Option<&str>) -> Result<Vec<u8>, RefusalPayload> {
    let shown = path.display().to_string();
    let to_refusal = |refusal: cmdrvl_tabular::Refusal| -> RefusalPayload {
        if refusal.code == TabularCode::EIo {
            return RefusalPayload::io(shown.clone(), refusal.detail);
        }
        let context = serde_json::to_value(&refusal.context)
            .unwrap_or_else(|_| serde_json::Value::Object(Default::default()));
        RefusalPayload::input_format(
            shown.clone(),
            refusal.code.as_str(),
            refusal.detail,
            context,
        )
    };
    let render_failure = || {
        RefusalPayload::io(
            shown.clone(),
            "could not render the input as CSV in memory".to_owned(),
        )
    };

    let opts = OpenOptions {
        sheet: sheet.map(SheetSel::parse),
        ..OpenOptions::default()
    };
    let mut table = open(path, &opts).map_err(to_refusal)?;
    debug_assert!(matches!(
        table.format(),
        Format::Tsv | Format::Jsonl | Format::Parquet | Format::Xlsx
    ));

    let mut writer = csv::WriterBuilder::new()
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());
    writer
        .write_record(table.headers())
        .map_err(|_| render_failure())?;
    for row in table.rows() {
        let row = row.map_err(to_refusal)?;
        writer
            .write_record(row.cells.iter().map(|cell| cell.as_bytes()))
            .map_err(|_| render_failure())?;
    }
    writer.into_inner().map_err(|_| render_failure())
}

/// Open a dataset for the CSV commands: CSV-like files are read directly, JSONL / Parquet /
/// XLSX are rendered through tabular first.
pub fn open_dataset(
    path: &Path,
    sheet: Option<&str>,
) -> Result<csv::Reader<Box<dyn Read>>, RefusalPayload> {
    let source: Box<dyn Read> =
        if is_tabular_path(path) {
            Box::new(Cursor::new(render_csv(path, sheet)?))
        } else {
            Box::new(File::open(path).map_err(|error| {
                RefusalPayload::io(path.display().to_string(), error.to_string())
            })?)
        };
    Ok(csv::Reader::from_reader(source))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_routing() {
        for yes in [
            "a.tsv",
            "a.jsonl",
            "a.NDJSON",
            "a.parquet",
            "a.xlsx",
            "a.XLSM",
            "a.xls",
        ] {
            assert!(is_tabular_path(Path::new(yes)), "{yes}");
        }
        for no in ["a.csv", "a.txt", "a", "a.csv.gz"] {
            assert!(!is_tabular_path(Path::new(no)), "{no}");
        }
    }
}
