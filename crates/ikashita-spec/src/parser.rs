//! KDL Application Profile v0.1 parser and semantic validation.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use serde_json::{Number, Value};

use crate::{
    diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Severity, SourceLocation},
    ir::{
        ActionDefinition, ActionStep, ActionStepKind, ApplicationDefinition, Component,
        ComponentKind, EventBinding, PageDefinition, ResourceCapability, ResourceDefinition,
        StateDefinition,
    },
    profile::{ApplicationProfile, MVP_PROFILE_VERSION, ProfileVersion},
};

#[derive(Clone, Copy, Debug)]
struct Span {
    offset: usize,
}

fn node_span(node: &KdlNode) -> Span {
    let span = node.span();
    Span { offset: span.offset() }
}

fn entry_span(entry: &KdlEntry) -> Span {
    let span = entry.span();
    Span { offset: span.offset() }
}

#[derive(Clone, Debug)]
enum Reference {
    Binding(BindingTarget),
    Resource(String),
    Action(String),
}

#[derive(Clone, Debug)]
enum BindingTarget {
    State(String),
    Form { id: String, field: String },
}

#[derive(Clone, Debug)]
struct ReferenceSite {
    reference: Reference,
    span: Span,
}

#[derive(Clone, Debug)]
struct FormInfo {
    id: Option<String>,
    fields: BTreeSet<String>,
}

#[derive(Clone, Default)]
struct Metadata {
    references: Vec<ReferenceSite>,
    ids: BTreeMap<String, Span>,
    forms: BTreeMap<usize, FormInfo>,
    next_form: usize,
}

struct ParseContext<'a> {
    source: &'a str,
    file: Option<String>,
    diagnostics: Diagnostics,
    metadata: Metadata,
}

impl<'a> ParseContext<'a> {
    fn new(source: &'a str, file: Option<String>) -> Self {
        Self { source, file, diagnostics: Diagnostics::new(), metadata: Metadata::default() }
    }

    fn location(&self, span: Span) -> SourceLocation {
        let offset = span.offset.min(self.source.len());
        let prefix = &self.source[..offset];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column_start = prefix.rfind('\n').map_or(0, |index| index + 1);
        let column = self.source[column_start..offset].chars().count() + 1;
        SourceLocation::new(self.file.clone(), line, column)
    }

    fn error(&mut self, code: DiagnosticCode, message: impl Into<String>, span: Span) {
        self.diagnostics.push(
            Diagnostic::new(code, Severity::Error, message).with_location(self.location(span)),
        );
    }

    fn reference(&mut self, reference: Reference, span: Span) {
        self.metadata.references.push(ReferenceSite { reference, span });
    }

    fn register_id(&mut self, id: &str, span: Span) {
        if self.metadata.ids.insert(id.to_owned(), span).is_some() {
            self.error(
                DiagnosticCode::DuplicateName,
                format!("component ID '{id}' is declared more than once"),
                span,
            );
        }
    }

    fn begin_form(&mut self, id: Option<String>) -> usize {
        let scope = self.metadata.next_form;
        self.metadata.next_form += 1;
        self.metadata.forms.insert(scope, FormInfo { id, fields: BTreeSet::new() });
        scope
    }

    fn register_field(&mut self, scope: Option<usize>, field: &str, span: Span) {
        let Some(scope) = scope else {
            self.error(
                DiagnosticCode::InvalidBinding,
                format!("field '{field}' must be declared inside a form"),
                span,
            );
            return;
        };
        let duplicate = self
            .metadata
            .forms
            .get_mut(&scope)
            .map(|form| !form.fields.insert(field.to_owned()))
            .unwrap_or(false);
        if duplicate {
            self.error(
                DiagnosticCode::DuplicateName,
                format!("form field '{field}' is declared more than once"),
                span,
            );
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HeaderStatus {
    Valid,
    Missing(usize),
    Unsupported { offset: usize, version: Option<String> },
}

fn header_status(source: &str) -> HeaderStatus {
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim();
        let line_offset = offset + line.len().saturating_sub(line.trim_start().len());
        offset += line.len();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if trimmed.starts_with("/-") {
            let parts: Vec<_> = trimmed.split_whitespace().collect();
            if parts.len() == 3 && parts[0] == "/-" && parts[1] == "kdl-version" {
                if parts[2] == "2" {
                    return HeaderStatus::Valid;
                }
                return HeaderStatus::Unsupported {
                    offset: line_offset,
                    version: Some(parts[2].to_owned()),
                };
            }
            return HeaderStatus::Unsupported { offset: line_offset, version: None };
        }
        return HeaderStatus::Missing(line_offset);
    }
    HeaderStatus::Missing(source.len())
}

/// Parses a KDL source string without reference validation.
pub(crate) fn parse(
    source: &str,
    file: Option<String>,
) -> Result<ApplicationDefinition, Diagnostics> {
    parse_internal(source, file, false)
}

/// Parses and validates a KDL source string.
pub(crate) fn parse_and_validate(
    source: &str,
    file: Option<String>,
) -> Result<ApplicationDefinition, Diagnostics> {
    parse_internal(source, file, true)
}

pub(crate) fn parse_file(path: &Path) -> Result<ApplicationDefinition, Diagnostics> {
    match fs::read_to_string(path) {
        Ok(source) => parse(&source, Some(path.display().to_string())),
        Err(error) => Err(io_diagnostics(path, error)),
    }
}

pub(crate) fn parse_and_validate_file(path: &Path) -> Result<ApplicationDefinition, Diagnostics> {
    match fs::read_to_string(path) {
        Ok(source) => parse_and_validate(&source, Some(path.display().to_string())),
        Err(error) => Err(io_diagnostics(path, error)),
    }
}

fn io_diagnostics(path: &Path, error: std::io::Error) -> Diagnostics {
    let mut diagnostics = Diagnostics::new();
    diagnostics.push(
        Diagnostic::new(
            DiagnosticCode::Io,
            Severity::Error,
            format!("could not read '{}': {error}", path.display()),
        )
        .with_location(SourceLocation::new(Some(path.display().to_string()), 1, 1)),
    );
    diagnostics
}

fn parse_internal(
    source: &str,
    file: Option<String>,
    validate_references: bool,
) -> Result<ApplicationDefinition, Diagnostics> {
    let mut context = ParseContext::new(source, file);
    match header_status(source) {
        HeaderStatus::Valid => {}
        HeaderStatus::Missing(offset) => context.error(
            DiagnosticCode::KdlHeaderMissing,
            "KDL v2 header '/- kdl-version 2' is required",
            Span { offset },
        ),
        HeaderStatus::Unsupported { offset, version } => context.error(
            DiagnosticCode::KdlVersionUnsupported,
            version.map_or_else(
                || "KDL version header must be '/- kdl-version 2'".to_owned(),
                |version| format!("unsupported KDL version '{version}'; expected '2'"),
            ),
            Span { offset },
        ),
    }

    let document = match source.parse::<KdlDocument>() {
        Ok(document) => document,
        Err(error) => {
            for diagnostic in error.diagnostics {
                let span = Span { offset: diagnostic.span.offset() };
                context.error(
                    DiagnosticCode::KdlParse,
                    diagnostic.message.unwrap_or_else(|| "invalid KDL syntax".to_owned()),
                    span,
                );
            }
            context.diagnostics.sort_deterministic();
            return Err(context.diagnostics);
        }
    };

    let Some(definition) = parse_document(&document, &mut context) else {
        context.diagnostics.sort_deterministic();
        return Err(context.diagnostics);
    };

    if validate_references && !context.diagnostics.has_errors() {
        let metadata = context.metadata.clone();
        validate_reference_sites(&definition, &metadata, &mut context);
    }

    context.diagnostics.sort_deterministic();
    if context.diagnostics.has_errors() { Err(context.diagnostics) } else { Ok(definition) }
}

fn parse_document(
    document: &KdlDocument,
    context: &mut ParseContext<'_>,
) -> Option<ApplicationDefinition> {
    let mut app_node = None;
    for node in document.nodes() {
        if node.name().value() == "app" {
            if app_node.is_some() {
                context.error(
                    DiagnosticCode::DuplicateName,
                    "the document contains more than one app node",
                    node_span(node),
                );
            } else {
                app_node = Some(node);
            }
        } else {
            context.error(
                DiagnosticCode::UnknownNode,
                format!("unknown top-level node '{}'", node.name().value()),
                node_span(node),
            );
        }
    }

    let app = app_node?;
    let attributes = attributes(app, &["version"], context);
    let name = string_argument(app, 0, context).filter(|name| !name.trim().is_empty());
    if name.is_none() {
        context.error(
            DiagnosticCode::MissingAttribute,
            "app requires one non-empty string name argument",
            node_span(app),
        );
    }
    check_argument_count(app, 1, 1, context);

    let Some(version_value) = attributes.get("version") else {
        context.error(
            DiagnosticCode::ProfileVersionMissing,
            "app requires version=\"0.1\"",
            node_span(app),
        );
        return None;
    };
    let version_text = string_value(version_value, attributes_span(app, "version"), context)?;
    let version = match version_text.parse::<ProfileVersion>() {
        Ok(version) if version == MVP_PROFILE_VERSION => version,
        Ok(version) => {
            context.error(
                DiagnosticCode::ProfileVersionUnsupported,
                format!("unsupported application profile version '{version}'; expected '0.1'"),
                attributes_span(app, "version"),
            );
            version
        }
        Err(()) => {
            context.error(
                DiagnosticCode::ProfileVersionUnsupported,
                format!("invalid application profile version '{version_text}'; expected '0.1'"),
                attributes_span(app, "version"),
            );
            MVP_PROFILE_VERSION
        }
    };

    let mut resources = Vec::new();
    let mut states = Vec::new();
    let mut pages = Vec::new();
    let mut actions = Vec::new();
    let mut names = BTreeMap::<&str, BTreeSet<String>>::new();
    let Some(children) = app.children() else {
        context.error(
            DiagnosticCode::MissingAttribute,
            "app requires a children block containing the application definition",
            node_span(app),
        );
        return Some(ApplicationDefinition {
            profile: ApplicationProfile { name: name.unwrap_or_default(), version },
            resources,
            states,
            pages,
            actions,
        });
    };

    for node in children.nodes() {
        let name_key = match node.name().value() {
            "resource" => "resource",
            "state" => "state",
            "page" => "page",
            "action" => "action",
            unknown => {
                context.error(
                    DiagnosticCode::UnknownNode,
                    format!("unknown app child node '{unknown}'"),
                    node_span(node),
                );
                continue;
            }
        };
        let name_set = names.entry(name_key).or_default();
        let parsed = match name_key {
            "resource" => parse_resource(node, context).map(|resource| {
                let duplicate = !name_set.insert(resource.name.clone());
                if duplicate {
                    context.error(
                        DiagnosticCode::DuplicateName,
                        format!("resource '{}' is declared more than once", resource.name),
                        node_span(node),
                    );
                }
                resources.push(resource);
            }),
            "state" => parse_state(node, context).map(|state| {
                let duplicate = !name_set.insert(state.name.clone());
                if duplicate {
                    context.error(
                        DiagnosticCode::DuplicateName,
                        format!("state '{}' is declared more than once", state.name),
                        node_span(node),
                    );
                }
                states.push(state);
            }),
            "page" => parse_page(node, context).map(|page| {
                let duplicate = !name_set.insert(page.name.clone());
                if duplicate {
                    context.error(
                        DiagnosticCode::DuplicateName,
                        format!("page '{}' is declared more than once", page.name),
                        node_span(node),
                    );
                }
                pages.push(page);
            }),
            "action" => parse_action(node, context).map(|action| {
                let duplicate = !name_set.insert(action.name.clone());
                if duplicate {
                    context.error(
                        DiagnosticCode::DuplicateName,
                        format!("action '{}' is declared more than once", action.name),
                        node_span(node),
                    );
                }
                actions.push(action);
            }),
            _ => None,
        };
        let _ = parsed;
    }

    Some(ApplicationDefinition {
        profile: ApplicationProfile { name: name.unwrap_or_default(), version },
        resources,
        states,
        pages,
        actions,
    })
}

fn parse_resource(node: &KdlNode, context: &mut ParseContext<'_>) -> Option<ResourceDefinition> {
    let resource_attributes = attributes(node, &["schema"], context);
    let name = string_argument(node, 0, context)?;
    if !is_safe_resource_name(&name) {
        context.error(
            DiagnosticCode::InvalidAttribute,
            format!("resource name '{name}' is not a safe API path segment"),
            node_span(node),
        );
    }
    check_argument_count(node, 1, 1, context);
    let schema = required_string_attribute(&resource_attributes, node, "schema", context)?;
    let mut required_capabilities = BTreeSet::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() != "require" {
                context.error(
                    DiagnosticCode::UnknownNode,
                    format!("unknown resource child node '{}'", child.name().value()),
                    node_span(child),
                );
                continue;
            }
            let _ = attributes(child, &[], context);
            let capability = string_argument(child, 0, context);
            check_argument_count(child, 1, 1, context);
            if let Some(capability) = capability {
                match resource_capability(&capability) {
                Some(capability) => {
                    if !required_capabilities.insert(capability) {
                        context.error(
                            DiagnosticCode::DuplicateName,
                            format!("resource capability '{capability}' is declared more than once"),
                            node_span(child),
                        );
                    }
                }
                None => context.error(
                    DiagnosticCode::InvalidEnum,
                    format!(
                        "invalid resource capability '{capability}'; expected list, get, create, update, delete, or invoke"
                    ),
                    node_span(child),
                ),
            }
            }
            if child.children().is_some() {
                context.error(
                    DiagnosticCode::UnknownNode,
                    "require nodes cannot have children",
                    node_span(child),
                );
            }
        }
    }
    Some(ResourceDefinition { name, schema, required_capabilities })
}

fn parse_state(node: &KdlNode, context: &mut ParseContext<'_>) -> Option<StateDefinition> {
    let attributes = attributes(node, &["value"], context);
    let name = string_argument(node, 0, context)?;
    check_argument_count(node, 1, 1, context);
    let property_value = attributes.get("value");
    let child_value = node.children().and_then(|children| {
        let values: Vec<_> =
            children.nodes().iter().filter(|child| child.name().value() == "value").collect();
        if values.len() > 1 {
            context.error(
                DiagnosticCode::DuplicateName,
                "state declares more than one value node",
                node_span(values[1]),
            );
        }
        values.first().copied()
    });
    if property_value.is_some() && child_value.is_some() {
        context.error(
            DiagnosticCode::DuplicateName,
            "state value must be declared as a property or a value child node, not both",
            node_span(node),
        );
    }
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() != "value" {
                context.error(
                    DiagnosticCode::UnknownNode,
                    format!("unknown state child node '{}'", child.name().value()),
                    node_span(child),
                );
            }
        }
    }
    let value = if let Some(raw) = property_value {
        kdl_value_to_json(raw, attributes_span(node, "value"), context)
    } else if let Some(child) = child_value {
        state_value_node(child, context)
    } else {
        context.error(
            DiagnosticCode::MissingAttribute,
            "state requires value=<value> or a value child node",
            node_span(node),
        );
        None
    }?;
    Some(StateDefinition { name, value })
}

fn state_value_node(node: &KdlNode, context: &mut ParseContext<'_>) -> Option<Value> {
    let args = positional_entries(node);
    if node.children().is_none() {
        if args.len() != 1 {
            context.error(
                DiagnosticCode::InvalidArguments,
                "value requires exactly one positional value",
                node_span(node),
            );
            return None;
        }
        return kdl_value_to_json(args[0].value(), entry_span(args[0]), context);
    }
    if !args.is_empty() {
        context.error(
            DiagnosticCode::InvalidArguments,
            "a structured value node cannot combine an argument with child values",
            node_span(node),
        );
        return None;
    }
    let children = node.children()?;
    let nodes = children.nodes();
    let array = nodes.iter().all(|child| child.name().value() == "-");
    if array {
        let mut values = Vec::new();
        for child in nodes {
            values.push(state_value_node(child, context)?);
        }
        return Some(Value::Array(values));
    }
    let mut object = serde_json::Map::new();
    for child in nodes {
        if object.contains_key(child.name().value()) {
            context.error(
                DiagnosticCode::DuplicateName,
                format!(
                    "structured state object key '{}' is declared more than once",
                    child.name().value()
                ),
                node_span(child),
            );
        }
        object.insert(child.name().value().to_owned(), state_value_node(child, context)?);
    }
    Some(Value::Object(object))
}

fn parse_page(node: &KdlNode, context: &mut ParseContext<'_>) -> Option<PageDefinition> {
    let attributes = attributes(node, &["title"], context);
    let name = string_argument(node, 0, context)?;
    check_argument_count(node, 1, 1, context);
    let title = required_string_attribute(&attributes, node, "title", context)?;
    let Some(children) = node.children() else {
        context.error(
            DiagnosticCode::MissingAttribute,
            "page requires a children block",
            node_span(node),
        );
        return Some(PageDefinition { name, title, components: Vec::new() });
    };
    let mut components = Vec::new();
    for child in children.nodes() {
        if child.name().value() == "on" {
            context.error(
                DiagnosticCode::UnknownNode,
                "on event nodes must be children of a component",
                node_span(child),
            );
            continue;
        }
        if let Some(component) = parse_component(child, false, None, context) {
            components.push(component);
        }
    }
    Some(PageDefinition { name, title, components })
}

fn parse_action(node: &KdlNode, context: &mut ParseContext<'_>) -> Option<ActionDefinition> {
    let attributes = attributes(node, &[], context);
    let name = string_argument(node, 0, context)?;
    check_argument_count(node, 1, 1, context);
    if !attributes.is_empty() {
        // Individual unknown-attribute diagnostics were emitted above.
    }
    let mut steps = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            let step = match child.name().value() {
                "validate" => parse_action_step(
                    child,
                    ActionStepKind::Validate,
                    &["target"],
                    &["target"],
                    context,
                ),
                "upsert" => parse_action_step(
                    child,
                    ActionStepKind::Upsert,
                    &["resource", "value"],
                    &["resource", "value"],
                    context,
                ),
                "refresh" => parse_action_step(
                    child,
                    ActionStepKind::Refresh,
                    &["resource"],
                    &["resource"],
                    context,
                ),
                "invoke" => parse_action_step(
                    child,
                    ActionStepKind::Invoke,
                    &["resource", "action", "input"],
                    &["resource", "action"],
                    context,
                ),
                "toast" => parse_toast_step(child, context),
                unknown => {
                    context.error(
                        DiagnosticCode::UnknownNode,
                        format!("unknown action step '{unknown}'"),
                        node_span(child),
                    );
                    None
                }
            };
            if let Some(step) = step {
                steps.push(step);
            }
        }
    }
    Some(ActionDefinition { name, steps })
}

fn parse_action_step(
    node: &KdlNode,
    kind: ActionStepKind,
    allowed: &[&str],
    required: &[&str],
    context: &mut ParseContext<'_>,
) -> Option<ActionStep> {
    let attributes = attributes(node, allowed, context);
    check_argument_count(node, 0, 0, context);
    if node.children().is_some() {
        context.error(
            DiagnosticCode::UnknownNode,
            "action steps cannot have children",
            node_span(node),
        );
    }
    for attribute in required {
        if !attributes.contains_key(*attribute) {
            context.error(
                DiagnosticCode::MissingAttribute,
                format!("action step '{}' requires {attribute}=...", node.name().value()),
                node_span(node),
            );
        } else {
            let _ = required_string_attribute(&attributes, node, attribute, context);
        }
    }
    if let Some(value) = attributes.get("input") {
        let _ = string_value(value, attributes_span(node, "input"), context);
    }
    let mut values = BTreeMap::new();
    for (name, value) in attributes {
        let Some(value) = kdl_value_to_json(&value, attributes_span(node, &name), context) else {
            continue;
        };
        if matches!(name.as_str(), "resource")
            && let Some(resource) = value.as_str()
        {
            context
                .reference(Reference::Resource(resource.to_owned()), attributes_span(node, &name));
        }
        values.insert(name, value);
    }
    Some(ActionStep { kind, attributes: values, text: None })
}

fn parse_toast_step(node: &KdlNode, context: &mut ParseContext<'_>) -> Option<ActionStep> {
    let attributes = attributes(node, &[], context);
    if !attributes.is_empty() {
        // Individual unknown-attribute diagnostics were emitted above.
    }
    let text = string_argument(node, 0, context);
    check_argument_count(node, 1, 1, context);
    if node.children().is_some() {
        context.error(
            DiagnosticCode::UnknownNode,
            "toast steps cannot have children",
            node_span(node),
        );
    }
    Some(ActionStep { kind: ActionStepKind::Toast, attributes: BTreeMap::new(), text })
}

fn parse_component(
    node: &KdlNode,
    table_child: bool,
    current_form: Option<usize>,
    context: &mut ParseContext<'_>,
) -> Option<Component> {
    if table_child && node.name().value() == "column" {
        return parse_table_column(node, context);
    }
    let (kind, allowed) = match node.name().value() {
        "column" => (ComponentKind::Column, &["id", "gap", "align"][..]),
        "row" => (ComponentKind::Row, &["id", "gap", "align"][..]),
        "text" => (ComponentKind::Text, &["id", "variant", "align"][..]),
        "text-input" => {
            (ComponentKind::TextInput, &["id", "label", "field", "bind", "variant"][..])
        }
        "select" => (ComponentKind::Select, &["id", "label", "field", "bind", "variant"][..]),
        "textarea" => (ComponentKind::Textarea, &["id", "label", "field", "bind", "variant"][..]),
        "button" => (ComponentKind::Button, &["id", "label", "action", "variant", "align"][..]),
        "data-table" => (ComponentKind::DataTable, &["id", "resource", "key", "bind"][..]),
        "form" => (ComponentKind::Form, &["id", "bind", "mode"][..]),
        unknown => {
            context.error(
                DiagnosticCode::UnknownNode,
                format!("unknown component '{unknown}'"),
                node_span(node),
            );
            return None;
        }
    };
    let raw_attributes = attributes(node, allowed, context);
    let mut attributes = BTreeMap::new();
    for (name, value) in &raw_attributes {
        let Some(value) = kdl_value_to_json(value, attributes_span(node, name), context) else {
            continue;
        };
        attributes.insert(name.clone(), value);
    }

    for enum_attribute in match kind {
        ComponentKind::Column | ComponentKind::Row => &["align"][..],
        ComponentKind::Text
        | ComponentKind::TextInput
        | ComponentKind::Select
        | ComponentKind::Textarea => &["variant"][..],
        ComponentKind::Button => &["variant", "align"][..],
        ComponentKind::Form => &["mode"][..],
        _ => &[][..],
    } {
        if let Some(value) = raw_attributes.get(*enum_attribute) {
            validate_enum_attribute(
                enum_attribute,
                value,
                attributes_span(node, enum_attribute),
                context,
            );
        }
    }
    if let Some(value) = raw_attributes.get("gap") {
        let Some(gap) = string_value(value, attributes_span(node, "gap"), context) else {
            continue_component_after_attribute_error(kind, node, context);
            return None;
        };
        if !matches!(gap.as_str(), "xs" | "sm" | "md" | "lg" | "xl") {
            context.error(
                DiagnosticCode::InvalidEnum,
                format!("invalid gap '{gap}'; expected xs, sm, md, lg, or xl"),
                attributes_span(node, "gap"),
            );
        }
    }

    let id = optional_string_attribute(&raw_attributes, node, "id", context);
    if let Some(id) = &id {
        context.register_id(id, attributes_span(node, "id"));
    }
    let text = match kind {
        ComponentKind::Text => {
            let text = string_argument(node, 0, context);
            check_argument_count(node, 1, 1, context);
            text
        }
        ComponentKind::Button => {
            let argument = string_argument(node, 0, context);
            check_argument_count(node, 0, 1, context);
            let label = optional_string_attribute(&raw_attributes, node, "label", context);
            if argument.is_some() && label.is_some() {
                context.error(
                    DiagnosticCode::InvalidAttribute,
                    "button label must be positional or label=..., not both",
                    node_span(node),
                );
            }
            if argument.is_none() && label.is_none() {
                context.error(
                    DiagnosticCode::MissingAttribute,
                    "button requires a label argument or label=...",
                    node_span(node),
                );
            }
            argument.or(label)
        }
        _ => {
            check_argument_count(node, 0, 0, context);
            None
        }
    };

    for attribute_name in ["field", "bind"] {
        if let Some(value) = raw_attributes.get(attribute_name) {
            let Some(value) = string_value(value, attributes_span(node, attribute_name), context)
            else {
                continue;
            };
            if attribute_name == "field" {
                context.register_field(current_form, &value, attributes_span(node, attribute_name));
            } else if let Some(binding) =
                parse_binding(&value, attributes_span(node, attribute_name), context)
            {
                context
                    .reference(Reference::Binding(binding), attributes_span(node, attribute_name));
            }
        }
    }
    if let Some(value) = raw_attributes.get("resource")
        && let Some(value) = string_value(value, attributes_span(node, "resource"), context)
    {
        context.reference(Reference::Resource(value), attributes_span(node, "resource"));
    }
    if let Some(value) = raw_attributes.get("action")
        && let Some(value) = string_value(value, attributes_span(node, "action"), context)
    {
        context.reference(Reference::Action(value), attributes_span(node, "action"));
    }
    if kind == ComponentKind::DataTable {
        let _ = required_string_attribute(&raw_attributes, node, "resource", context);
        let _ = required_string_attribute(&raw_attributes, node, "key", context);
    }
    let child_form = if kind == ComponentKind::Form {
        let scope = context.begin_form(id.clone());
        if let Some(form) = context.metadata.forms.get_mut(&scope) {
            form.id = id.clone();
        }
        Some(scope)
    } else {
        current_form
    };
    let mut children = Vec::new();
    let mut events = Vec::new();
    if let Some(document) = node.children() {
        for child in document.nodes() {
            if child.name().value() == "on" {
                if let Some(event) = parse_event(child, context) {
                    events.push(event);
                }
                continue;
            }
            let child_allowed = match kind {
                ComponentKind::DataTable => child.name().value() == "column",
                ComponentKind::Column | ComponentKind::Row | ComponentKind::Form => true,
                _ => false,
            };
            if !child_allowed {
                context.error(
                    DiagnosticCode::UnknownNode,
                    format!(
                        "component '{}' cannot contain '{}'",
                        kind.as_str(),
                        child.name().value()
                    ),
                    node_span(child),
                );
                continue;
            }
            if let Some(component) =
                parse_component(child, kind == ComponentKind::DataTable, child_form, context)
            {
                children.push(component);
            }
        }
    }

    Some(Component { kind, id, text, attributes, children, events })
}

fn continue_component_after_attribute_error(
    _kind: ComponentKind,
    _node: &KdlNode,
    _context: &mut ParseContext<'_>,
) {
    // The caller only uses this helper to make the control flow explicit when
    // a required scalar attribute already produced a diagnostic.
}

fn parse_table_column(node: &KdlNode, context: &mut ParseContext<'_>) -> Option<Component> {
    let raw_attributes = attributes(node, &["id", "field", "label"], context);
    check_argument_count(node, 0, 0, context);
    if node.children().is_some() {
        context.error(
            DiagnosticCode::UnknownNode,
            "data-table columns cannot have children",
            node_span(node),
        );
    }
    let field = required_string_attribute(&raw_attributes, node, "field", context);
    let id = optional_string_attribute(&raw_attributes, node, "id", context);
    if let Some(id) = &id {
        context.register_id(id, attributes_span(node, "id"));
    }
    let mut attributes = BTreeMap::new();
    for (name, value) in raw_attributes {
        if let Some(value) = kdl_value_to_json(&value, attributes_span(node, &name), context) {
            attributes.insert(name, value);
        }
    }
    Some(Component {
        kind: ComponentKind::TableColumn,
        id,
        text: field,
        attributes,
        children: Vec::new(),
        events: Vec::new(),
    })
}

fn parse_event(node: &KdlNode, context: &mut ParseContext<'_>) -> Option<EventBinding> {
    let raw_attributes = attributes(node, &["action"], context);
    let event = string_argument(node, 0, context)?;
    check_argument_count(node, 1, 1, context);
    let action = required_string_attribute(&raw_attributes, node, "action", context)?;
    if node.children().is_some() {
        context.error(
            DiagnosticCode::UnknownNode,
            "on event nodes cannot have children",
            node_span(node),
        );
    }
    context.reference(Reference::Action(action.clone()), attributes_span(node, "action"));
    Some(EventBinding { event, action })
}

fn parse_binding(value: &str, span: Span, context: &mut ParseContext<'_>) -> Option<BindingTarget> {
    if let Some(state) = value.strip_prefix("state.")
        && !state.is_empty()
    {
        return Some(BindingTarget::State(state.to_owned()));
    }
    if let Some(form) = value.strip_prefix("form.") {
        let mut parts = form.splitn(2, '.');
        let id = parts.next().unwrap_or_default();
        let field = parts.next().unwrap_or_default();
        if !id.is_empty() && !field.is_empty() {
            return Some(BindingTarget::Form { id: id.to_owned(), field: field.to_owned() });
        }
    }
    context.error(
        DiagnosticCode::InvalidBinding,
        format!("invalid binding '{value}'; expected state.<name> or form.<id>.<field>"),
        span,
    );
    None
}

fn validate_reference_sites(
    definition: &ApplicationDefinition,
    metadata: &Metadata,
    context: &mut ParseContext<'_>,
) {
    let states: BTreeSet<_> = definition.states.iter().map(|state| state.name.as_str()).collect();
    let resources: BTreeSet<_> =
        definition.resources.iter().map(|resource| resource.name.as_str()).collect();
    let actions: BTreeSet<_> =
        definition.actions.iter().map(|action| action.name.as_str()).collect();
    let forms: BTreeMap<_, _> = metadata
        .forms
        .values()
        .filter_map(|form| form.id.as_deref().map(|id| (id, &form.fields)))
        .collect();
    for site in &metadata.references {
        match &site.reference {
            Reference::Binding(BindingTarget::State(name)) if !states.contains(name.as_str()) => {
                context.error(
                    DiagnosticCode::UnknownState,
                    format!("binding targets undeclared state '{name}'"),
                    site.span,
                );
            }
            Reference::Binding(BindingTarget::Form { id, field }) => {
                if !forms.get(id.as_str()).is_some_and(|fields| fields.contains(field)) {
                    context.error(
                        DiagnosticCode::InvalidBinding,
                        format!("binding targets undeclared field '{field}' in form '{id}'"),
                        site.span,
                    );
                }
            }
            Reference::Binding(BindingTarget::State(_)) => {}
            Reference::Resource(name) if !resources.contains(name.as_str()) => {
                context.error(
                    DiagnosticCode::UnknownResource,
                    format!("reference targets undeclared resource '{name}'"),
                    site.span,
                );
            }
            Reference::Resource(_) => {}
            Reference::Action(name) if !actions.contains(name.as_str()) => {
                context.error(
                    DiagnosticCode::UnknownAction,
                    format!("reference targets undeclared action '{name}'"),
                    site.span,
                );
            }
            Reference::Action(_) => {}
        }
    }
}

pub(crate) fn validate(definition: &ApplicationDefinition) -> Result<(), Diagnostics> {
    let mut diagnostics = Diagnostics::new();
    if definition.profile.version != MVP_PROFILE_VERSION {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::ProfileVersionUnsupported,
            Severity::Error,
            format!(
                "unsupported application profile version '{}'; expected '0.1'",
                definition.profile.version
            ),
        ));
    }
    if definition.profile.name.trim().is_empty() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingAttribute,
            Severity::Error,
            "application name must not be empty",
        ));
    }
    validate_unique_names(
        definition.resources.iter().map(|resource| resource.name.as_str()),
        "resource",
        &mut diagnostics,
    );
    validate_unique_names(
        definition.states.iter().map(|state| state.name.as_str()),
        "state",
        &mut diagnostics,
    );
    validate_unique_names(
        definition.pages.iter().map(|page| page.name.as_str()),
        "page",
        &mut diagnostics,
    );
    validate_unique_names(
        definition.actions.iter().map(|action| action.name.as_str()),
        "action",
        &mut diagnostics,
    );
    for resource in &definition.resources {
        if !is_safe_resource_name(&resource.name) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidAttribute,
                Severity::Error,
                format!("resource name '{}' is not a safe API path segment", resource.name),
            ));
        }
        if resource.schema.trim().is_empty() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MissingAttribute,
                Severity::Error,
                format!("resource '{}' requires a non-empty schema", resource.name),
            ));
        }
    }
    let resources: BTreeSet<String> =
        definition.resources.iter().map(|resource| resource.name.clone()).collect();
    let states: BTreeSet<String> =
        definition.states.iter().map(|state| state.name.clone()).collect();
    let actions: BTreeSet<String> =
        definition.actions.iter().map(|action| action.name.clone()).collect();
    let mut ids = BTreeSet::new();
    let mut forms = BTreeMap::new();
    {
        let mut validator = ComponentValidator {
            resources: &resources,
            actions: &actions,
            ids: &mut ids,
            forms: &mut forms,
            diagnostics: &mut diagnostics,
        };
        for page in &definition.pages {
            validate_components(&page.components, None, &mut validator);
        }
    }
    for action in &definition.actions {
        for step in &action.steps {
            for key in ["resource"] {
                if let Some(resource) = step.attributes.get(key).and_then(Value::as_str)
                    && !resources.contains(resource)
                {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::UnknownResource,
                        Severity::Error,
                        format!(
                            "action '{}' references undeclared resource '{resource}'",
                            action.name
                        ),
                    ));
                }
            }
        }
    }
    validate_bindings_against_forms(&definition.pages, &states, &forms, &mut diagnostics);
    diagnostics.sort_deterministic();
    if diagnostics.has_errors() { Err(diagnostics) } else { Ok(()) }
}

fn is_safe_resource_name(name: &str) -> bool {
    !name.is_empty()
        && name.trim() == name
        && name != "."
        && name != ".."
        && !name.chars().any(|character| character.is_control() || matches!(character, '/' | '\\'))
}

fn validate_unique_names<'a>(
    names: impl Iterator<Item = &'a str>,
    kind: &str,
    diagnostics: &mut Diagnostics,
) {
    let mut seen = BTreeSet::new();
    for name in names {
        if !seen.insert(name) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DuplicateName,
                Severity::Error,
                format!("{kind} '{name}' is declared more than once"),
            ));
        }
    }
}

struct ComponentValidator<'a> {
    resources: &'a BTreeSet<String>,
    actions: &'a BTreeSet<String>,
    ids: &'a mut BTreeSet<String>,
    forms: &'a mut BTreeMap<String, BTreeSet<String>>,
    diagnostics: &'a mut Diagnostics,
}

fn validate_components(
    components: &[Component],
    form: Option<&str>,
    validator: &mut ComponentValidator<'_>,
) {
    for component in components {
        if let Some(id) = &component.id
            && !validator.ids.insert(id.clone())
        {
            validator.diagnostics.push(Diagnostic::new(
                DiagnosticCode::DuplicateName,
                Severity::Error,
                format!("component ID '{id}' is declared more than once"),
            ));
        }
        if component.kind == ComponentKind::DataTable && component.string_attribute("key").is_none()
        {
            validator.diagnostics.push(Diagnostic::new(
                DiagnosticCode::MissingAttribute,
                Severity::Error,
                "data-table requires key=...",
            ));
        }
        if let Some(resource) = component.string_attribute("resource")
            && !validator.resources.contains(resource)
        {
            validator.diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnknownResource,
                Severity::Error,
                format!("component references undeclared resource '{resource}'"),
            ));
        }
        if let Some(action) = component.string_attribute("action")
            && !validator.actions.contains(action)
        {
            validator.diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnknownAction,
                Severity::Error,
                format!("component references undeclared action '{action}'"),
            ));
        }
        if let Some(field) = component.string_attribute("field") {
            if form.is_none() {
                validator.diagnostics.push(Diagnostic::new(
                    DiagnosticCode::InvalidBinding,
                    Severity::Error,
                    format!("field '{field}' must be declared inside a form"),
                ));
            } else if let Some(form_id) = form {
                let duplicate = !validator
                    .forms
                    .entry(form_id.to_owned())
                    .or_default()
                    .insert(field.to_owned());
                if duplicate {
                    validator.diagnostics.push(Diagnostic::new(
                        DiagnosticCode::DuplicateName,
                        Severity::Error,
                        format!("form field '{field}' is declared more than once"),
                    ));
                }
            }
        }
        let next_form = if component.kind == ComponentKind::Form {
            let form_id = component.string_attribute("id").unwrap_or("").to_owned();
            validator.forms.entry(form_id.clone()).or_default();
            Some(form_id)
        } else {
            form.map(str::to_owned)
        };
        validate_components(&component.children, next_form.as_deref(), validator);
        for event in &component.events {
            if !validator.actions.contains(event.action.as_str()) {
                validator.diagnostics.push(Diagnostic::new(
                    DiagnosticCode::UnknownAction,
                    Severity::Error,
                    format!(
                        "event '{}' references undeclared action '{}'",
                        event.event, event.action
                    ),
                ));
            }
        }
    }
}

fn validate_bindings_against_forms(
    pages: &[PageDefinition],
    states: &BTreeSet<String>,
    forms: &BTreeMap<String, BTreeSet<String>>,
    diagnostics: &mut Diagnostics,
) {
    fn visit(
        components: &[Component],
        states: &BTreeSet<String>,
        forms: &BTreeMap<String, BTreeSet<String>>,
        diagnostics: &mut Diagnostics,
    ) {
        for component in components {
            if let Some(binding) = component.string_attribute("bind") {
                match parse_binding_text(binding) {
                    Some(BindingTarget::State(name)) if !states.contains(name.as_str()) => {
                        diagnostics.push(Diagnostic::new(
                            DiagnosticCode::UnknownState,
                            Severity::Error,
                            format!("binding targets undeclared state '{name}'"),
                        ))
                    }
                    Some(BindingTarget::Form { id, field })
                        if !forms.get(&id).is_some_and(|fields| fields.contains(&field)) =>
                    {
                        diagnostics.push(Diagnostic::new(
                            DiagnosticCode::InvalidBinding,
                            Severity::Error,
                            format!("binding targets undeclared field '{field}' in form '{id}'"),
                        ))
                    }
                    None => diagnostics.push(Diagnostic::new(
                        DiagnosticCode::InvalidBinding,
                        Severity::Error,
                        format!("invalid binding '{binding}'"),
                    )),
                    _ => {}
                }
            }
            visit(&component.children, states, forms, diagnostics);
        }
    }
    for page in pages {
        visit(&page.components, states, forms, diagnostics);
    }
}

fn parse_binding_text(value: &str) -> Option<BindingTarget> {
    if let Some(name) = value.strip_prefix("state.")
        && !name.is_empty()
    {
        return Some(BindingTarget::State(name.to_owned()));
    }
    if let Some(rest) = value.strip_prefix("form.") {
        let (id, field) = rest.split_once('.')?;
        if !id.is_empty() && !field.is_empty() {
            return Some(BindingTarget::Form { id: id.to_owned(), field: field.to_owned() });
        }
    }
    None
}

fn attributes(
    node: &KdlNode,
    allowed: &[&str],
    context: &mut ParseContext<'_>,
) -> BTreeMap<String, KdlValue> {
    let mut values = BTreeMap::new();
    for entry in node.entries() {
        let Some(name) = entry.name() else { continue };
        let name = name.value().to_owned();
        if !allowed.contains(&name.as_str()) {
            context.error(
                DiagnosticCode::UnknownAttribute,
                format!("unknown attribute '{name}' on node '{}'", node.name().value()),
                entry_span(entry),
            );
            continue;
        }
        if values.insert(name.clone(), entry.value().clone()).is_some() {
            context.error(
                DiagnosticCode::DuplicateName,
                format!("attribute '{name}' is declared more than once"),
                entry_span(entry),
            );
        }
    }
    values
}

fn attributes_span(node: &KdlNode, name: &str) -> Span {
    node.entries()
        .iter()
        .find(|entry| entry.name().is_some_and(|key| key.value() == name))
        .map_or_else(|| node_span(node), entry_span)
}

fn positional_entries(node: &KdlNode) -> Vec<&KdlEntry> {
    node.entries().iter().filter(|entry| entry.name().is_none()).collect()
}

fn check_argument_count(
    node: &KdlNode,
    minimum: usize,
    maximum: usize,
    context: &mut ParseContext<'_>,
) {
    let count = positional_entries(node).len();
    if count < minimum || count > maximum {
        let expected = if minimum == maximum {
            format!("exactly {minimum}")
        } else {
            format!("between {minimum} and {maximum}")
        };
        context.error(
            DiagnosticCode::InvalidArguments,
            format!(
                "node '{}' requires {expected} positional argument(s), found {count}",
                node.name().value()
            ),
            node_span(node),
        );
    }
}

fn string_argument(node: &KdlNode, index: usize, context: &mut ParseContext<'_>) -> Option<String> {
    let entry = positional_entries(node).get(index).copied()?;
    string_value(entry.value(), entry_span(entry), context)
}

fn string_value(value: &KdlValue, span: Span, context: &mut ParseContext<'_>) -> Option<String> {
    match value {
        KdlValue::String(value) => Some(value.clone()),
        _ => {
            context.error(DiagnosticCode::InvalidAttribute, "expected a string value", span);
            None
        }
    }
}

fn required_string_attribute(
    values: &BTreeMap<String, KdlValue>,
    node: &KdlNode,
    name: &str,
    context: &mut ParseContext<'_>,
) -> Option<String> {
    let Some(value) = values.get(name) else {
        context.error(
            DiagnosticCode::MissingAttribute,
            format!("node '{}' requires {name}=...", node.name().value()),
            node_span(node),
        );
        return None;
    };
    let value = string_value(value, attributes_span(node, name), context)?;
    if value.trim().is_empty() {
        context.error(
            DiagnosticCode::InvalidAttribute,
            format!("attribute '{name}' must not be empty"),
            attributes_span(node, name),
        );
        return None;
    }
    Some(value)
}

fn optional_string_attribute(
    values: &BTreeMap<String, KdlValue>,
    node: &KdlNode,
    name: &str,
    context: &mut ParseContext<'_>,
) -> Option<String> {
    let value = values.get(name)?;
    let value = string_value(value, attributes_span(node, name), context)?;
    if value.trim().is_empty() {
        context.error(
            DiagnosticCode::InvalidAttribute,
            format!("attribute '{name}' must not be empty"),
            attributes_span(node, name),
        );
        return None;
    }
    Some(value)
}

fn validate_enum_attribute(
    name: &str,
    value: &KdlValue,
    span: Span,
    context: &mut ParseContext<'_>,
) {
    let Some(value) = string_value(value, span, context) else { return };
    let valid = match name {
        "align" => matches!(value.as_str(), "start" | "center" | "end" | "stretch"),
        "variant" => {
            matches!(value.as_str(), "default" | "primary" | "secondary" | "danger" | "ghost")
        }
        "mode" => matches!(value.as_str(), "inline" | "drawer" | "dialog"),
        _ => true,
    };
    if !valid {
        context.error(DiagnosticCode::InvalidEnum, format!("invalid {name} value '{value}'"), span);
    }
}

fn resource_capability(value: &str) -> Option<ResourceCapability> {
    match value {
        "list" => Some(ResourceCapability::List),
        "get" => Some(ResourceCapability::Get),
        "create" => Some(ResourceCapability::Create),
        "update" => Some(ResourceCapability::Update),
        "delete" => Some(ResourceCapability::Delete),
        "invoke" => Some(ResourceCapability::Invoke),
        _ => None,
    }
}

fn kdl_value_to_json(
    value: &KdlValue,
    span: Span,
    context: &mut ParseContext<'_>,
) -> Option<Value> {
    match value {
        KdlValue::String(value) => {
            let trimmed = value.trim_start();
            if (trimmed.starts_with('{') || trimmed.starts_with('['))
                && let Ok(json) = serde_json::from_str::<Value>(value)
                && (json.is_object() || json.is_array())
            {
                return Some(json);
            }
            Some(Value::String(value.clone()))
        }
        KdlValue::Integer(value) => Number::from_i128(*value).map(Value::Number).or_else(|| {
            context.error(
                DiagnosticCode::InvalidStateValue,
                "integer value is outside the supported JSON number range",
                span,
            );
            None
        }),
        KdlValue::Float(value) => Number::from_f64(*value).map(Value::Number).or_else(|| {
            context.error(
                DiagnosticCode::InvalidStateValue,
                "non-finite floating-point values are not valid JSON",
                span,
            );
            None
        }),
        KdlValue::Bool(value) => Some(Value::Bool(*value)),
        KdlValue::Null => Some(Value::Null),
    }
}
