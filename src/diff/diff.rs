use serde_json::Value;

use crate::cli::args::DiffArgs;
use crate::refusal::RefusalPayload;

pub fn handle(_a: &str, _b: &str) -> Result<Value, RefusalPayload> {
    todo!()
}

pub fn run(args: &DiffArgs, _no_witness: bool) -> Result<Value, RefusalPayload> {
    handle(&args.a, &args.b)
}
