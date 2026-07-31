//! Server-side boundaries for validated applications and static bundles.
//!
//! The foundation defines configuration and bundle values without selecting an
//! HTTP framework. This keeps transport choice separate from the Resource
//! Contract and allows a future WASM/static server to share the same metadata.

pub mod bundle;
pub mod config;

pub use bundle::StaticBundle;
pub use config::ServerConfig;
