use serde_json::Value;

use crate::cli::args::LintArgs;
use crate::refusal::RefusalPayload;

pub fn run(_args: &LintArgs, _no_witness: bool) -> Result<Value, RefusalPayload> {
    todo!("lint handler")
}
