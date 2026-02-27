use serde_json::Value;

use crate::cli::args::ShowArgs;
use crate::refusal::RefusalPayload;

pub fn handle(_profile_id: &str) -> Result<Value, RefusalPayload> {
    todo!()
}

pub fn run(args: &ShowArgs, _no_witness: bool) -> Result<Value, RefusalPayload> {
    handle(&args.profile_id)
}
