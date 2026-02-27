use serde_json::Value;

use crate::cli::args::StatsArgs;
use crate::refusal::RefusalPayload;

pub fn run(_args: &StatsArgs, _no_witness: bool) -> Result<Value, RefusalPayload> {
    todo!("stats implementation")
}
