use serde_json::Value;

use crate::cli::args::DraftNewArgs;
use crate::refusal::RefusalPayload;

pub fn run(_args: &DraftNewArgs, _no_witness: bool) -> Result<Value, RefusalPayload> {
    todo!("draft new handler")
}
