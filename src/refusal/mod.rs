// Refusal system + codes will be implemented by bd-1r7
// This placeholder allows cargo check to pass

#[derive(Debug)]
pub struct RefusalPayload {
    pub code: String,
    pub detail: String,
}

impl std::fmt::Display for RefusalPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for RefusalPayload {}
