use serde_json::Value;

use crate::cli::args::DraftInitArgs;
use crate::refusal::RefusalPayload;

pub fn run(_args: &DraftInitArgs, _no_witness: bool) -> Result<Value, RefusalPayload> {
    todo!("draft init handler")
}
