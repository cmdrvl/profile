use serde_json::Value;

use crate::cli::args::SuggestKeyArgs;
use crate::refusal::RefusalPayload;

pub fn run(_args: &SuggestKeyArgs, _no_witness: bool) -> Result<Value, RefusalPayload> {
    todo!("suggest-key implementation")
}