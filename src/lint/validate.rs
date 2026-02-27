use serde_json::Value;

use crate::cli::args::ValidateArgs;
use crate::refusal::RefusalPayload;

pub fn run(_args: &ValidateArgs, _no_witness: bool) -> Result<Value, RefusalPayload> {
    todo!("validate handler")
}
