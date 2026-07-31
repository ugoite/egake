//! Resource schema and capability model.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    /// An integral numeric value.
    Integer,
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
    /// JSON Schema enum values, when the field is constrained to a set.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "enum")]
    pub enum_values: Option<Vec<Value>>,
    /// A supported JSON Schema format such as `email`, `date`, or `date-time`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

impl FieldSchema {
    /// Creates an optional field declaration.
    #[must_use]
    pub fn new(name: impl Into<String>, field_type: FieldType) -> Self {
        Self { name: name.into(), field_type, required: false, enum_values: None, format: None }
    }

    /// Marks this field as required.
    #[must_use]
    pub const fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Adds JSON Schema enum values to this field.
    #[must_use]
    pub fn with_enum_values(mut self, values: Vec<Value>) -> Self {
        self.enum_values = Some(values);
        self
    }

    /// Adds a supported JSON Schema format to this field.
    #[must_use]
    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.format = Some(format.into());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_metadata_is_additive_on_the_json_boundary() {
        let legacy = serde_json::to_value(FieldSchema::new("name", FieldType::Text))
            .expect("legacy field JSON");
        assert_eq!(
            legacy,
            serde_json::json!({
                "name": "name",
                "field_type": "text",
                "required": false,
            })
        );

        let enriched = FieldSchema::new("status", FieldType::Text)
            .with_enum_values(vec![serde_json::json!("active")])
            .with_format("email");
        let value = serde_json::to_value(enriched).expect("enriched field JSON");
        assert_eq!(value["enum"], serde_json::json!(["active"]));
        assert_eq!(value["format"], "email");
    }
}
