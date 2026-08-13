//! Lowering from Application Profile nodes to the Ikasue Web ABI.
//!
//! This is the only UI boundary owned by Egake. `IkaView` is a wire value for
//! Ikasue; resource, state, and action metadata stays in `UiBinding`.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::{Value, json};

use crate::ir::{ApplicationDefinition, PageDefinition, ViewNode};

/// The versioned Web Platform contract consumed by Ikasue Custom Elements.
pub const IKASUE_ABI_VERSION: &str = "ikasue-web/1";

/// A JSON-safe Ikasue view tree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IkaView {
    /// Version of the view contract.
    pub version: &'static str,
    /// Ikasue Custom Element kind, for example `stack` or `data-grid`.
    pub kind: String,
    /// UI-only properties. Egake resource/action names never appear here.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub props: BTreeMap<String, Value>,
    /// Child views.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Self>,
    /// Text content for text-like views.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// A named page view sent to Ikasue.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IkaPage {
    /// Stable Application Profile page name.
    pub name: String,
    /// Human-readable page title.
    pub title: String,
    /// Root Ikasue view.
    pub view: IkaView,
}

/// Egake-owned metadata attached to a DOM target.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UiBinding {
    /// Stable DOM target ID.
    pub target: String,
    /// Egake binding category.
    pub kind: String,
    /// DOM event to observe.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Action name handled by Egake.
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Resource name handled by Egake.
    pub resource: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Resource primary key field.
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// State or form binding expression.
    pub bind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Form field name.
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// DataGrid column metadata.
    pub columns: Option<Vec<Value>>,
    /// Optional form target for an action binding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<String>,
}

/// The compiler output consumed by the Egake browser host and Ikasue.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LoweredUi {
    /// ABI version.
    pub version: &'static str,
    /// Lowered page views.
    pub views: Vec<IkaPage>,
    /// Egake-owned bindings.
    pub bindings: Vec<UiBinding>,
}

/// Lower every validated page deterministically.
#[must_use]
pub fn lower_application(definition: &ApplicationDefinition) -> LoweredUi {
    let mut pages = definition.pages.clone();
    pages.sort_by(|left, right| left.name.cmp(&right.name));
    let mut used_ids = BTreeSet::new();
    for page in &pages {
        collect_explicit_ids(&page.views, &mut used_ids);
    }
    let mut bindings = Vec::new();
    let views = pages
        .iter()
        .enumerate()
        .map(|(page_index, page)| lower_page(page, page_index, &mut bindings, &mut used_ids))
        .collect();
    LoweredUi { version: IKASUE_ABI_VERSION, views, bindings }
}

fn lower_page(
    page: &PageDefinition,
    page_index: usize,
    bindings: &mut Vec<UiBinding>,
    used_ids: &mut BTreeSet<String>,
) -> IkaPage {
    let children = page
        .views
        .iter()
        .enumerate()
        .map(|(index, node)| {
            lower_node(node, &format!("page-{page_index}-{index}"), None, bindings, used_ids)
        })
        .collect::<Vec<_>>();
    let page_id = allocate_synthetic_id(&format!("page-{}", page.name), used_ids);
    IkaPage {
        name: page.name.clone(),
        title: page.title.clone(),
        view: IkaView {
            version: IKASUE_ABI_VERSION,
            kind: "stack".to_owned(),
            props: BTreeMap::from([(String::from("id"), Value::String(page_id))]),
            children,
            text: None,
        },
    }
}

fn lower_node(
    node: &ViewNode,
    path: &str,
    form_scope: Option<&str>,
    bindings: &mut Vec<UiBinding>,
    used_ids: &mut BTreeSet<String>,
) -> IkaView {
    let target = node.id.clone().unwrap_or_else(|| allocate_synthetic_id(path, used_ids));
    let kind = match node.name.as_str() {
        "column" => "stack",
        "row" => "flex",
        "text" => "text",
        "text-input" => "text-field",
        "select" => "select",
        "textarea" => "text-field",
        "button" => "icon-button",
        "data-table" => "data-grid",
        "form" => "form",
        _ => "text",
    };
    let mut props = BTreeMap::new();
    props.insert("id".to_owned(), Value::String(target.clone()));
    for (name, value) in &node.attributes {
        if !matches!(name.as_str(), "resource" | "action" | "key" | "field" | "bind" | "form") {
            props.insert(name.clone(), value.clone());
        }
    }
    if node.name == "select" {
        props.insert("editor".to_owned(), Value::String("select".to_owned()));
    } else if node.name == "textarea" {
        props.insert("editor".to_owned(), Value::String("textarea".to_owned()));
    }

    if let Some(bind) = node.string_attribute("bind") {
        bindings.push(UiBinding {
            target: target.clone(),
            kind: "value".to_owned(),
            event: Some("change".to_owned()),
            action: None,
            resource: None,
            key: None,
            bind: Some(bind.to_owned()),
            field: node.string_attribute("field").map(str::to_owned),
            columns: None,
            form: None,
        });
    } else if let Some(field) = node.string_attribute("field") {
        bindings.push(UiBinding {
            target: target.clone(),
            kind: "field".to_owned(),
            event: Some("change".to_owned()),
            action: None,
            resource: None,
            key: None,
            bind: None,
            field: Some(field.to_owned()),
            columns: None,
            form: None,
        });
    }
    if let Some(resource) = node.string_attribute("resource") {
        let columns = if node.name == "data-table" {
            Some(
                node.children
                    .iter()
                    .map(|column| {
                        json!({
                            "id": column.id.clone().unwrap_or_else(|| column.string_attribute("field").unwrap_or("column").to_owned()),
                            "label": column.string_attribute("label").unwrap_or_else(|| column.string_attribute("field").unwrap_or("Column")),
                            "field": column.string_attribute("field").unwrap_or_default(),
                        })
                    })
                    .collect(),
            )
        } else {
            None
        };
        bindings.push(UiBinding {
            target: target.clone(),
            kind: "resource".to_owned(),
            event: Some("ika-query".to_owned()),
            action: None,
            resource: Some(resource.to_owned()),
            key: node.string_attribute("key").map(str::to_owned),
            bind: None,
            field: None,
            columns,
            form: None,
        });
    }
    let action_form = node.string_attribute("form").or(form_scope);
    if let Some(action) = node.string_attribute("action") {
        bindings.push(UiBinding {
            target: target.clone(),
            kind: "action".to_owned(),
            event: Some("ika-action".to_owned()),
            action: Some(action.to_owned()),
            resource: None,
            key: None,
            bind: None,
            field: None,
            columns: None,
            form: action_form.map(str::to_owned),
        });
    }
    for event in &node.events {
        let event_name = dom_event_name(&event.event);
        let duplicate = node.string_attribute("action").is_some()
            && matches!(event_name.as_str(), "ika-action" | "click")
            && node.string_attribute("action") == Some(event.action.as_str());
        if !duplicate {
            bindings.push(UiBinding {
                target: target.clone(),
                kind: "action".to_owned(),
                event: Some(event_name),
                action: Some(event.action.clone()),
                resource: None,
                key: None,
                bind: None,
                field: None,
                columns: None,
                form: event.form.as_deref().or(action_form).map(str::to_owned),
            });
        }
    }

    let children = if node.name == "data-table" {
        Vec::new()
    } else {
        node.children
            .iter()
            .enumerate()
            .map(|(index, child)| {
                lower_node(
                    child,
                    &format!("{path}-{index}"),
                    (node.name == "form").then_some(target.as_str()).or(form_scope),
                    bindings,
                    used_ids,
                )
            })
            .collect()
    };
    IkaView {
        version: IKASUE_ABI_VERSION,
        kind: kind.to_owned(),
        props,
        children,
        text: node.text.clone(),
    }
}

fn collect_explicit_ids(views: &[ViewNode], ids: &mut BTreeSet<String>) {
    for view in views {
        if let Some(id) = &view.id {
            ids.insert(id.clone());
        }
        collect_explicit_ids(&view.children, ids);
    }
}

fn allocate_synthetic_id(candidate: &str, used_ids: &mut BTreeSet<String>) -> String {
    if used_ids.insert(candidate.to_owned()) {
        return candidate.to_owned();
    }
    let mut suffix = 2;
    loop {
        let id = format!("{candidate}-{suffix}");
        if used_ids.insert(id.clone()) {
            return id;
        }
        suffix += 1;
    }
}

fn dom_event_name(name: &str) -> String {
    match name {
        "action" => "ika-action".to_owned(),
        "edit" => "ika-edit".to_owned(),
        "query" => "ika-query".to_owned(),
        "select" => "ika-select".to_owned(),
        other => other.to_owned(),
    }
}
