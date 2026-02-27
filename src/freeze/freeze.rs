use serde_json::Value;

use crate::cli::args::FreezeArgs;
use crate::refusal::RefusalPayload;

pub fn run(_args: &FreezeArgs, _no_witness: bool) -> Result<Value, RefusalPayload> {
    todo!("freeze handler")
}
