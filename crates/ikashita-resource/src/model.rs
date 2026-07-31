//! Resource schema and capability model.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// An operation a provider may expose to the runtime.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Read the resource schema.
    Schema,
    /// List matching records.
    List,
    /// Read one record.
    Get,
    /// Create a record.
    Create,
    /// Apply a merge patch to a record.
    Update,
    /// Delete a record.
    Delete,
    /// Invoke a provider-defined action.
    Invoke,
}

/// The supported primitive field kinds in the foundation schema.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    /// UTF-8 text.
    Text,
    /// A numeric value.
    Number,
    /// A Boolean value.
    Boolean,
    /// An ISO-8601 date or date-time value.
    Date,
    /// A provider-defined structured value.
    Json,
}

/// A field declaration used for validation and form generation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FieldSchema {
    /// Field name as used by the resource record.
    pub name: String,
    /// Primitive or structured field kind.
    pub field_type: FieldType,
    /// Whether a value is required when creating or updating a record.
    pub required: bool,
}

impl FieldSchema {
    /// Creates an optional field declaration.
    #[must_use]
    pub fn new(name: impl Into<String>, field_type: FieldType) -> Self {
        Self { name: name.into(), field_type, required: false }
    }

    /// Marks this field as required.
    #[must_use]
    pub const fn required(mut self) -> Self {
        self.required = true;
        self
    }
}

/// The schema and capabilities advertised by one resource.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceSchema {
    /// Stable resource name used by the application definition.
    pub name: String,
    /// Fields available on records in this resource.
    pub fields: Vec<FieldSchema>,
    /// Operations granted by the provider.
    pub capabilities: BTreeSet<Capability>,
}

impl ResourceSchema {
    /// Creates an empty schema for a named resource.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), fields: Vec::new(), capabilities: BTreeSet::new() }
    }

    /// Appends a field declaration.
    pub fn push_field(&mut self, field: FieldSchema) {
        self.fields.push(field);
    }

    /// Grants one provider capability.
    pub fn grant(&mut self, capability: Capability) {
        self.capabilities.insert(capability);
    }
}
