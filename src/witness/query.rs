use serde_json::Value;

use crate::cli::args::{WitnessCountArgs, WitnessLastArgs, WitnessQueryArgs};
use crate::refusal::RefusalPayload;

pub fn run_query(_args: &WitnessQueryArgs) -> Result<Value, RefusalPayload> {
    todo!("witness query implementation")
}

pub fn run_last(_args: &WitnessLastArgs) -> Result<Value, RefusalPayload> {
    todo!("witness last implementation")
}

pub fn run_count(_args: &WitnessCountArgs) -> Result<Value, RefusalPayload> {
    todo!("witness count implementation")
}
