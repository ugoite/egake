//! The provider traits shared by storage and host-language adapters.

use std::sync::{Mutex, MutexGuard};

use serde_json::Value;

use crate::{
    ListQuery, ResourceError, ResourceErrorKind, ResourcePage, ResourceResult, ResourceSchema,
};

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

/// An object-safe JSON provider boundary for transport adapters.
///
/// Unlike [`ResourceProvider`], this trait uses `serde_json::Value` and shared
/// references. Providers can therefore be registered behind `Arc` and handle
/// concurrent HTTP requests with their own internal synchronization.
pub trait JsonResourceProvider: Send + Sync {
    /// Returns the provider schema and granted capabilities.
    fn schema(&self) -> ResourceResult<ResourceSchema>;

    /// Lists matching JSON objects using offset/limit pagination.
    fn list(&self, query: &ListQuery) -> ResourceResult<ResourcePage<Value>>;

    /// Gets one JSON object by its stable identifier.
    fn get(&self, id: &str) -> ResourceResult<Option<Value>>;

    /// Creates one JSON object and returns the stored value.
    fn create(&self, value: Value) -> ResourceResult<Value>;

    /// Applies an object-shaped merge patch and returns the stored value.
    fn update(&self, id: &str, patch: Value) -> ResourceResult<Value>;

    /// Deletes one record by its stable identifier.
    fn delete(&self, id: &str) -> ResourceResult<()>;

    /// Invokes a provider-defined action, when supported.
    fn invoke(&self, _action: &str, _input: Value) -> ResourceResult<Value> {
        Err(ResourceError::new(
            ResourceErrorKind::CapabilityDenied,
            "provider does not expose actions",
        ))
    }
}

/// An object-safe adapter for an existing generic JSON-valued provider.
pub struct JsonResourceProviderAdapter<P> {
    provider: Mutex<P>,
}

impl<P> JsonResourceProviderAdapter<P> {
    /// Wraps a generic provider for shared JSON dispatch.
    #[must_use]
    pub fn new(provider: P) -> Self {
        Self { provider: Mutex::new(provider) }
    }

    /// Returns a mutable reference to the wrapped provider when it is not shared.
    pub fn get_mut(&mut self) -> ResourceResult<&mut P> {
        self.provider.get_mut().map_err(|_| lock_error())
    }

    fn lock(&self) -> ResourceResult<MutexGuard<'_, P>> {
        self.provider.lock().map_err(|_| lock_error())
    }
}

impl<P> JsonResourceProvider for JsonResourceProviderAdapter<P>
where
    P: ResourceProvider<Item = Value> + Send + Sync,
{
    fn schema(&self) -> ResourceResult<ResourceSchema> {
        self.lock()?.schema()
    }

    fn list(&self, query: &ListQuery) -> ResourceResult<ResourcePage<Value>> {
        self.lock()?.list(query)
    }

    fn get(&self, id: &str) -> ResourceResult<Option<Value>> {
        self.lock()?.get(id)
    }

    fn create(&self, value: Value) -> ResourceResult<Value> {
        self.lock()?.create(value)
    }

    fn update(&self, id: &str, patch: Value) -> ResourceResult<Value> {
        require_object_patch(&patch)?;
        self.lock()?.update(id, patch)
    }

    fn delete(&self, id: &str) -> ResourceResult<()> {
        self.lock()?.delete(id)
    }
}

fn lock_error() -> ResourceError {
    ResourceError::new(ResourceErrorKind::Internal, "provider lock is poisoned")
}

/// Applies an RFC 7396-style JSON merge patch and returns the merged value.
///
/// A non-object patch replaces the target. Object members containing `null`
/// remove keys, and nested objects are merged recursively. The patch itself
/// must be an object for resource updates; this standalone function also
/// supports scalar and array replacement so it can be reused by adapters.
pub fn apply_merge_patch(mut target: Value, patch: &Value) -> ResourceResult<Value> {
    merge_patch_in_place(&mut target, patch);
    Ok(target)
}

fn merge_patch_in_place(target: &mut Value, patch: &Value) {
    let Value::Object(patch_object) = patch else {
        *target = patch.clone();
        return;
    };

    if !target.is_object() {
        *target = Value::Object(serde_json::Map::new());
    }
    let Value::Object(target_object) = target else { unreachable!() };

    for (key, value) in patch_object {
        if value.is_null() {
            target_object.remove(key);
        } else {
            let entry = target_object.entry(key.clone()).or_insert(Value::Null);
            merge_patch_in_place(entry, value);
        }
    }
}

/// Validates that a value is an object-shaped merge patch.
pub fn require_object_patch(patch: &Value) -> ResourceResult<()> {
    if patch.is_object() {
        Ok(())
    } else {
        Err(ResourceError::new(
            ResourceErrorKind::Validation,
            "resource update patch must be a JSON object",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_patch_recurses_and_removes_null_members() {
        let target = serde_json::json!({
            "name": "Ada",
            "profile": {"department": "math", "active": true},
            "tags": ["admin"]
        });
        let patch = serde_json::json!({
            "profile": {"department": "science", "active": null},
            "tags": ["research"],
            "new_field": 3
        });
        let merged = apply_merge_patch(target, &patch).expect("merge");
        assert_eq!(merged["profile"]["department"], "science");
        assert!(merged["profile"].get("active").is_none());
        assert_eq!(merged["tags"], serde_json::json!(["research"]));
        assert_eq!(merged["new_field"], 3);
    }

    #[test]
    fn scalar_merge_patch_replaces_target_but_resource_patch_requires_object() {
        assert_eq!(
            apply_merge_patch(serde_json::json!({"a": 1}), &serde_json::json!(4)).unwrap(),
            4
        );
        assert_eq!(
            require_object_patch(&serde_json::json!(null)).unwrap_err().kind,
            ResourceErrorKind::Validation
        );
    }
}
