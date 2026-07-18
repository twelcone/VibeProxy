//! Usage tracking: poll Anthropic's OAuth usage endpoint per profile and surface it to UI + tray.

pub mod client;
pub mod model;
pub mod poller;

pub use model::{ProfileUsage, UsageStatus};
pub use poller::UsageState;
