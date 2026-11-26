//! Data-fabric push/pull. The HTTP client lives behind the opt-in `network` Cargo feature (off by
//! default, bd-t1r): the default binary has no HTTP or TLS crates, and `push` / `pull` refuse
//! with `E_NETWORK_DISABLED` before reading any file.

pub mod pull;
pub mod push;

pub use pull::handle_pull;
pub use push::handle_push;

#[cfg(feature = "network")]
mod http;
#[cfg(feature = "network")]
pub(crate) use http::{get_text, post_json};

/// Without the `network` feature there is no client: these stand-ins only keep `handle_push` and
/// `handle_pull` compiling; `push::run` / `pull::run` refuse before they are reached.
#[cfg(not(feature = "network"))]
pub(crate) fn post_json(
    _path: &str,
    _body: &serde_json::Value,
) -> Result<String, crate::refusal::RefusalPayload> {
    Err(crate::refusal::RefusalPayload::network_disabled())
}

#[cfg(not(feature = "network"))]
pub(crate) fn get_text(_path: &str) -> Result<String, crate::refusal::RefusalPayload> {
    Err(crate::refusal::RefusalPayload::network_disabled())
}
