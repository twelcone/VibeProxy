//! Usage fetching + the per-profile usage model. The background poller (timer + events) is an app
//! concern and lives in the desktop crate.

pub mod client;
pub mod model;

pub use model::{ProfileUsage, UsageStatus};
