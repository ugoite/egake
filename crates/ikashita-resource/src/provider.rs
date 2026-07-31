//! The provider trait shared by storage and host-language adapters.

use crate::{ListQuery, ResourcePage, ResourceResult, ResourceSchema};

/// The CRUD contract implemented by a resource adapter.
///
/// Methods are synchronous at this foundation layer so the crate remains
/// runtime-neutral. HTTP, WASM, and host-language adapters can schedule these
/// operations asynchronously without changing the value and error contract.
pub trait ResourceProvider {
    /// The record type owned by this provider.
    type Item: Clone;

    /// Returns the provider schema and granted capabilities.
    fn schema(&self) -> ResourceResult<ResourceSchema>;

    /// Lists matching records using offset/limit pagination.
    fn list(&self, query: &ListQuery) -> ResourceResult<ResourcePage<Self::Item>>;

    /// Gets one record by its stable identifier.
    fn get(&self, id: &str) -> ResourceResult<Option<Self::Item>>;

    /// Creates one record and returns the stored value.
    fn create(&mut self, value: Self::Item) -> ResourceResult<Self::Item>;

    /// Applies an object-shaped merge patch and returns the stored value.
    fn update(&mut self, id: &str, patch: Self::Item) -> ResourceResult<Self::Item>;

    /// Deletes one record by its stable identifier.
    fn delete(&mut self, id: &str) -> ResourceResult<()>;
}

/// An optional extension for providers that expose domain-specific actions.
pub trait ResourceActionProvider: ResourceProvider {
    /// The input accepted by a domain action.
    type ActionInput;
    /// The output returned by a domain action.
    type ActionOutput;

    /// Invokes one provider-defined action.
    fn invoke(
        &mut self,
        action: &str,
        input: Self::ActionInput,
    ) -> ResourceResult<Self::ActionOutput>;
}
