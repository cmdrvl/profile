use std::path::Path;

use serde_json::Value;

use crate::refusal::payload::RefusalPayload;

pub fn handle_push(_file: &Path) -> Result<Value, RefusalPayload> {
    todo!("bd-1cb implements push");
}
