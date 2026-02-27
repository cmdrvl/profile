use serde_json::Value;

use crate::cli::args::ListArgs;
use crate::refusal::RefusalPayload;

pub fn run(_args: &ListArgs, _no_witness: bool) -> Result<Value, RefusalPayload> {
    todo!("list implementation")
}