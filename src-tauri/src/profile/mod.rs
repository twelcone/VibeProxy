//! Profile data model, on-disk store, and path resolution.

pub mod account_meta;
pub mod paths;
pub mod store;

pub use store::{Config, Profile, Settings};
