//! Public API for DataZen database driver plugins.
//!
//! External drivers depend on this crate and implement [`DatabaseDriver`].
//! They register themselves at link time via the [`register_driver!`] macro,
//! which uses the [`inventory`] crate for zero-code discovery in the host binary.

pub use async_trait::async_trait;
pub use inventory;

mod types;
mod traits;
mod factory;
mod reuse;
pub mod command;
pub mod sql_dump;

pub use command::*;
pub use types::*;
pub use traits::*;
pub use factory::*;
pub use reuse::ReuseDriver;

/// Protocol version for the driver API.
pub const PROTOCOL_VERSION: u32 = 1;

/// Minimum protocol version the host still supports.
pub const MIN_PROTOCOL_VERSION: u32 = 1;
