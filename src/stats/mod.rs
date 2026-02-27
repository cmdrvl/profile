#[allow(clippy::module_inception)]
pub mod stats;
pub mod suggest_key;

pub use stats::run as run_stats;
pub use suggest_key::run as run_suggest_key;
