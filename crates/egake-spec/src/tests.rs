use super::*;

const CONTACTS: &str = include_str!("../../../examples/contacts-crud/app.ui.kdl");

#[test]
fn parses_valid_contacts_crud_definition() {
    let definition = parse_and_validate(CONTACTS).expect("contacts fixture should validate");

    assert_eq!(definition.profile.name, "contacts-admin");
    assert_eq!(definition.resources[0].name, "contacts");
    assert_eq!(definition.resources[0].required_capabilities.len(), 5);
    assert_eq!(definition.states[2].value["sort"], "name");
    assert_eq!(definition.pages[0].views[0].name, "column");
    assert_eq!(definition.actions.len(), 4);
}

#[test]
fn lowers_ui_to_ikasue_views_without_egake_metadata_in_props() {
    let definition = parse_and_validate(CONTACTS).expect("contacts fixture should validate");
    let first = lower_application(&definition);
    let second = lower_application(&definition);
    assert_eq!(first, second, "lowering must be deterministic");
    let encoded = serde_json::to_string(&first).expect("lowered UI JSON");
    assert!(encoded.contains("ikasue-web/1"));
    assert!(encoded.contains("data-grid"));
    assert!(first.bindings.iter().any(|binding| {
        binding.kind == "resource"
            && binding.resource.as_deref() == Some("contacts")
            && binding.event.as_deref() == Some("ika-query")
    }));
    assert!(first.bindings.iter().any(|binding| {
        binding.kind == "action"
            && binding.action.as_deref() == Some("open-edit")
            && binding.event.as_deref() == Some("ika-select")
    }));
    fn assert_view(view: &IkaView) {
        assert!(!view.props.contains_key("resource"));
        assert!(!view.props.contains_key("action"));
        for child in &view.children {
            assert_view(child);
        }
    }
    for page in first.views {
        assert_view(&page.view);
    }
}

#[test]
fn reserves_synthetic_ids_against_explicit_view_ids() {
    let source = r#"/- kdl-version 2
app "demo" version="0.1" {
    page "main" title="Main" {
        text id="page-0-1" "explicit"
        text id="page-main" "page id collision"
        row {
            text "generated"
        }
    }
}
"#;
    let definition = parse_and_validate(source).expect("definition should validate");
    let lowered = lower_application(&definition);
    let root = &lowered.views[0].view;
    let mut ids = std::collections::BTreeSet::new();
    fn collect(view: &IkaView, ids: &mut std::collections::BTreeSet<String>) {
        let id = view.props.get("id").and_then(serde_json::Value::as_str).expect("view ID");
        assert!(ids.insert(id.to_owned()), "duplicate lowered ID {id}");
        for child in &view.children {
            collect(child, ids);
        }
    }
    collect(root, &mut ids);
    assert!(ids.contains("page-0-1"));
    assert_ne!(root.props.get("id").and_then(serde_json::Value::as_str), Some("page-main"));
}

#[test]
fn rejects_missing_kdl_header() {
    let source = CONTACTS.replacen("/- kdl-version 2\n", "", 1);
    let diagnostics = parse_and_validate(&source).expect_err("header is required");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::KdlHeaderMissing
            && diagnostic.location.as_ref().is_some_and(|location| location.line == 1)
    }));
}

#[test]
fn rejects_unsupported_kdl_version() {
    let source = CONTACTS.replacen("/- kdl-version 2", "/- kdl-version 1", 1);
    let diagnostics = parse_and_validate(&source).expect_err("KDL v1 is not supported");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::KdlVersionUnsupported
            && diagnostic.message.contains("expected '2'")
    }));
}

#[test]
fn rejects_unknown_components_and_attributes() {
    let source = r#"/- kdl-version 2
app "demo" version="0.1" {
    page "main" title="Main" {
        mystery
        text "hello" bogus="value"
    }
}"#;
    let diagnostics = parse(source).expect_err("unknown profile elements are invalid");

    assert!(diagnostics.iter().any(|diagnostic| diagnostic.code == DiagnosticCode::UnknownNode));
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.code == DiagnosticCode::UnknownAttribute)
    );
}

#[test]
fn rejects_unknown_view_nodes_in_constructed_ir() {
    let source = r#"/- kdl-version 2
app "demo" version="0.1" {
    page "main" title="Main" {
        text "hello"
    }
}
"#;
    let mut definition = parse(source).expect("source shape should parse");
    definition.pages[0].views[0].name = "future-widget".to_owned();
    let diagnostics = definition.validate().expect_err("unknown IR view must be rejected");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::UnknownNode
            && diagnostic.message.contains("future-widget")
    }));
}

#[test]
fn rejects_invalid_enums_and_duplicate_component_ids() {
    let source = r#"/- kdl-version 2
app "demo" version="0.1" {
    page "main" title="Main" {
        row id="toolbar" align="sideways"
        text id="toolbar" "duplicate"
    }
}
"#;
    let diagnostics = parse(source).expect_err("invalid enum and duplicate ID are invalid");

    assert!(diagnostics.iter().any(|diagnostic| diagnostic.code == DiagnosticCode::InvalidEnum));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::DuplicateName && diagnostic.message.contains("toolbar")
    }));
}

#[test]
fn rejects_bad_resource_references() {
    let source = r#"/- kdl-version 2
app "demo" version="0.1" {
    action "load"
    page "main" title="Main" {
        data-table resource="missing" key="id"
    }
}
"#;
    let diagnostics = parse_and_validate(source).expect_err("missing resource is invalid");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::UnknownResource && diagnostic.message.contains("missing")
    }));
}

#[test]
fn rejects_resource_names_that_are_ambiguous_in_http_paths() {
    let source = r#"/- kdl-version 2
app "demo" version="0.1" {
    resource "../contacts" schema="schemas/contacts.json" {
        require "list"
    }
}
"#;
    let diagnostics = parse_and_validate(source).expect_err("unsafe resource name");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::InvalidAttribute
            && diagnostic.message.contains("safe API path segment")
    }));
}

#[test]
fn rejects_duplicate_state_names() {
    let source = r#"/- kdl-version 2
app "demo" version="0.1" {
    state "query" value=""
    state "query" value="again"
}
"#;
    let diagnostics = parse(source).expect_err("duplicate state is invalid");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::DuplicateName
            && diagnostic.message.contains("state 'query'")
    }));
}

#[test]
fn rejects_unknown_action_references() {
    let source = r#"/- kdl-version 2
app "demo" version="0.1" {
    page "main" title="Main" {
        button "Save" action="save"
    }
}
"#;
    let diagnostics = parse_and_validate(source).expect_err("unknown action is invalid");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::UnknownAction && diagnostic.message.contains("save")
    }));
}

#[test]
fn validates_field_bindings_against_declared_form_fields() {
    let source = r#"/- kdl-version 2
app "demo" version="0.1" {
    state "draft" value=#null
    page "main" title="Main" {
        form id="editor" bind="state.draft" {
            text-input field="name"
        }
        text-input bind="form.editor.email"
    }
}
"#;
    let diagnostics = parse_and_validate(source).expect_err("field binding is invalid");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::InvalidBinding && diagnostic.message.contains("email")
    }));
}

#[test]
fn preserves_and_validates_explicit_action_form_targets() {
    let source = r#"/- kdl-version 2
app "demo" version="0.1" {
    resource "contacts" schema="schemas/contacts.json" {
        require "list"
    }
    action "open-edit"
    page "main" title="Main" {
        data-table resource="contacts" key="id" form="editor" {
            column field="name" label="Name"
            on "select" action="open-edit" form="editor"
        }
        form id="editor" {
            text-input field="name"
        }
    }
}
"#;
    let definition = parse_and_validate(source).expect("explicit form target should validate");
    let lowered = lower_application(&definition);
    assert!(lowered.bindings.iter().any(|binding| {
        binding.event.as_deref() == Some("ika-select")
            && binding.action.as_deref() == Some("open-edit")
            && binding.form.as_deref() == Some("editor")
    }));

    let diagnostics = parse_and_validate(&source.replace("form=\"editor\"", "form=\"missing\""))
        .expect_err("unknown form target should be rejected");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::InvalidBinding && diagnostic.message.contains("missing")
    }));
}

#[test]
fn accepts_json_ish_state_values() {
    let source = r#"/- kdl-version 2
app "demo" version="0.1" {
    state "object" value="{\"enabled\":true,\"count\":2}"
    state "array" {
        value {
            - "first"
            - 2
            - #false
        }
    }
}
"#;
    let definition = parse_and_validate(source).expect("JSON-ish state values should validate");

    assert_eq!(definition.states[0].value["enabled"], true);
    assert_eq!(definition.states[1].value, serde_json::json!(["first", 2, false]));
}

#[test]
fn diagnostic_formatting_is_deterministic() {
    let source = r#"app "demo" version="0.1" {
    page "main" title="Main" {
        text "hello" bad="one"
    }
}
"#;
    let first = parse(source).expect_err("missing header and unknown attribute are invalid");
    let second = parse(source).expect_err("same source should render identically");

    assert_eq!(first.render(), second.render());
    assert_eq!(
        first.render(),
        "error IK1002: <input>:1:1: KDL v2 header '/- kdl-version 2' is required\nerror IK2005: <input>:3:22: unknown attribute 'bad' on node 'text'"
    );
}
