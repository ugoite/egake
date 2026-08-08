//! Owned typed intermediate representation for KDL Application Profile v0.1.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::Path,
};

use serde_json::Value;

use crate::{diagnostic::Diagnostics, parser, profile::ApplicationProfile};

/// A transport-neutral resource capability requested by an application.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResourceCapability {
    /// List records.
    List,
    /// Read one record.
    Get,
    /// Create a record.
    Create,
    /// Update a record using the resource contract's patch semantics.
    Update,
    /// Delete a record.
    Delete,
    /// Invoke a provider-defined action.
    Invoke,
}

impl ResourceCapability {
    /// Returns the KDL spelling of this capability.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Get => "get",
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Invoke => "invoke",
        }
    }
}

impl fmt::Display for ResourceCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One resource requested by an application definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceDefinition {
    /// Stable resource name used by components and actions.
    pub name: String,
    /// Path or identifier for the resource schema.
    pub schema: String,
    /// Capabilities the application expects its provider to expose.
    pub required_capabilities: BTreeSet<ResourceCapability>,
}

/// One named application state value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateDefinition {
    /// State name used by `state.<name>` bindings.
    pub name: String,
    /// JSON-compatible initial value.
    pub value: Value,
}

/// A page and its component tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageDefinition {
    /// Stable page name.
    pub name: String,
    /// Human-readable page title.
    pub title: String,
    /// Top-level components rendered on this page.
    pub components: Vec<Component>,
}

/// The known component kinds in the v0.1 MVP.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ComponentKind {
    /// Vertical layout container.
    Column,
    /// Horizontal layout container.
    Row,
    /// Static text.
    Text,
    /// Single-line text input.
    TextInput,
    /// Select input.
    Select,
    /// Multiline text input.
    Textarea,
    /// User-triggered action button.
    Button,
    /// Resource-backed table.
    DataTable,
    /// A field-editing form.
    Form,
    /// A column declaration inside a data table.
    TableColumn,
}

impl ComponentKind {
    /// Returns the KDL node name for this component kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Column => "column",
            Self::Row => "row",
            Self::Text => "text",
            Self::TextInput => "text-input",
            Self::Select => "select",
            Self::Textarea => "textarea",
            Self::Button => "button",
            Self::DataTable => "data-table",
            Self::Form => "form",
            Self::TableColumn => "column",
        }
    }
}

/// One component in a page's owned tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Component {
    /// Known renderer component kind.
    pub kind: ComponentKind,
    /// Optional stable component ID.
    pub id: Option<String>,
    /// Optional positional text, such as a button label.
    pub text: Option<String>,
    /// Known component attributes represented as owned JSON primitives.
    pub attributes: BTreeMap<String, Value>,
    /// Nested components.
    pub children: Vec<Self>,
    /// Event/action pairs declared below this component.
    pub events: Vec<EventBinding>,
}

impl Component {
    /// Returns a component attribute by its profile name.
    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&Value> {
        self.attributes.get(name)
    }

    /// Returns a string component attribute, if it exists and is a string.
    #[must_use]
    pub fn string_attribute(&self, name: &str) -> Option<&str> {
        self.attribute(name).and_then(Value::as_str)
    }
}

/// One event/action pair attached to a component.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventBinding {
    /// Event name, for example `select`.
    pub event: String,
    /// Top-level action name to invoke.
    pub action: String,
}

/// One declared action known to the application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionDefinition {
    /// Stable action name referenced by buttons and events.
    pub name: String,
    /// Optional declarative action steps.
    pub steps: Vec<ActionStep>,
}

/// Known declarative action step kinds.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ActionStepKind {
    /// Validate a target value.
    Validate,
    /// Create or update a resource value.
    Upsert,
    /// Refresh a resource list.
    Refresh,
    /// Display a message.
    Toast,
    /// Invoke a resource-defined action.
    Invoke,
}

/// One declarative action step with owned renderer/runtime metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionStep {
    /// Known action step kind.
    pub kind: ActionStepKind,
    /// Typed step attributes.
    pub attributes: BTreeMap<String, Value>,
    /// Optional positional text, used by `toast`.
    pub text: Option<String>,
}

/// A complete owned application definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationDefinition {
    /// Validated profile identity and application name.
    pub profile: ApplicationProfile,
    /// Declared resources.
    pub resources: Vec<ResourceDefinition>,
    /// Declared initial state values.
    pub states: Vec<StateDefinition>,
    /// Declared pages and component trees.
    pub pages: Vec<PageDefinition>,
    /// Declared actions.
    pub actions: Vec<ActionDefinition>,
}

impl ApplicationDefinition {
    /// Parses a KDL source string into an owned definition.
    pub fn parse(source: &str) -> Result<Self, Diagnostics> {
        parser::parse(source, None)
    }

    /// Parses a named KDL source string into an owned definition.
    pub fn parse_named(source: &str, file: impl Into<String>) -> Result<Self, Diagnostics> {
        parser::parse(source, Some(file.into()))
    }

    /// Reads and parses a KDL file into an owned definition.
    pub fn parse_file(path: impl AsRef<Path>) -> Result<Self, Diagnostics> {
        parser::parse_file(path.as_ref())
    }

    /// Parses and semantically validates a KDL source string.
    pub fn parse_and_validate(source: &str) -> Result<Self, Diagnostics> {
        parser::parse_and_validate(source, None)
    }

    /// Parses and semantically validates a named KDL source string.
    pub fn parse_and_validate_named(
        source: &str,
        file: impl Into<String>,
    ) -> Result<Self, Diagnostics> {
        parser::parse_and_validate(source, Some(file.into()))
    }

    /// Reads, parses, and semantically validates a KDL file.
    pub fn parse_and_validate_file(path: impl AsRef<Path>) -> Result<Self, Diagnostics> {
        parser::parse_and_validate_file(path.as_ref())
    }

    /// Validates references and invariants on an already-owned definition.
    pub fn validate(&self) -> Result<(), Diagnostics> {
        parser::validate(self)
    }
}
