//! App-side usage: the background poller (timer, shared state, events, tray). The fetch client and
//! the `ProfileUsage` model live in `vibeproxy_core::usage`.

pub mod poller;

pub use poller::UsageState;
pub use vibeproxy_core::usage::{ProfileUsage, UsageStatus};
