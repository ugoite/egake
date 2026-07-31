//! Shared, transport-neutral Resource Contract types.
//!
//! The contract is deliberately independent of HTTP, storage, serialization,
//! and async runtimes. An adapter can translate these values to its own
//! transport while preserving the same semantics.

pub mod error;
pub mod model;
pub mod provider;
pub mod query;

pub use error::{ResourceError, ResourceErrorKind, ResourceResult};
pub use model::{Capability, FieldSchema, FieldType, ResourceSchema};
pub use provider::{
    JsonResourceProvider, JsonResourceProviderAdapter, ResourceActionProvider, ResourceProvider,
    apply_merge_patch, require_object_patch,
};
pub use query::{ListQuery, ResourcePage, Sort, SortDirection};
