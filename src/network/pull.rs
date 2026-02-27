use std::path::Path;

use serde_json::Value;

use crate::refusal::payload::RefusalPayload;

pub fn handle_pull(_profile_id: &str, _out: &Path) -> Result<Value, RefusalPayload> {
    todo!("bd-1cb implements pull");
}
