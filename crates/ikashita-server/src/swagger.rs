//! The standalone HTTP API contract and its dependency-free Swagger UI.

use serde_json::{Map, Value, json};

const OPENAPI_VERSION: &str = "3.0.3";

/// The route suffix used to retrieve the OpenAPI document.
pub(crate) const OPENAPI_JSON_SUFFIX: &str = "/openapi.json";
/// The route suffix used to open the local Swagger UI.
pub(crate) const SWAGGER_UI_SUFFIX: &str = "/swagger";
/// The trailing-slash form accepted for the local Swagger UI.
pub(crate) const SWAGGER_UI_SLASH_SUFFIX: &str = "/swagger/";
/// The explicit index form accepted for the local Swagger UI.
pub(crate) const SWAGGER_UI_INDEX_SUFFIX: &str = "/swagger/index.html";

#[derive(Clone, Copy)]
enum ParameterKind {
    Path,
    QueryString,
    QueryInteger { minimum: u64, maximum: Option<u64>, default: u64 },
}

#[derive(Clone, Copy)]
struct ParameterContract {
    name: &'static str,
    kind: ParameterKind,
    description: &'static str,
}

#[derive(Clone, Copy)]
enum SchemaContract {
    AnyJson,
    JsonObject,
    ResourcePage,
    ResourceSchema,
    DeleteResponse,
}

#[derive(Clone, Copy)]
struct RequestBodyContract {
    description: &'static str,
    schema: SchemaContract,
    content_types: &'static [&'static str],
}

#[derive(Clone, Copy)]
pub(crate) struct EndpointContract {
    operation_id: &'static str,
    method: &'static str,
    path: &'static str,
    summary: &'static str,
    description: &'static str,
    parameters: &'static [ParameterContract],
    request_body: Option<RequestBodyContract>,
    success_status: &'static str,
    success_description: &'static str,
    success_schema: SchemaContract,
}

const RESOURCE: ParameterContract = ParameterContract {
    name: "resource",
    kind: ParameterKind::Path,
    description: "Registered resource name; it is one safe URL path segment.",
};
const ITEM_ID: ParameterContract = ParameterContract {
    name: "id",
    kind: ParameterKind::Path,
    description: "Resource item identifier; percent-decoded once as one path segment.",
};
const ACTION: ParameterContract = ParameterContract {
    name: "action",
    kind: ParameterKind::Path,
    description: "Provider-defined action name; it is one safe URL path segment.",
};
const Q: ParameterContract = ParameterContract {
    name: "q",
    kind: ParameterKind::QueryString,
    description: "Optional provider-defined text search.",
};
const SORT: ParameterContract = ParameterContract {
    name: "sort",
    kind: ParameterKind::QueryString,
    description: "Comma-separated field names; prefix with '-' or suffix with ':desc' for descending order.",
};
const OFFSET: ParameterContract = ParameterContract {
    name: "offset",
    kind: ParameterKind::QueryInteger { minimum: 0, maximum: None, default: 0 },
    description: "Number of matching records to skip.",
};
const LIMIT: ParameterContract = ParameterContract {
    name: "limit",
    kind: ParameterKind::QueryInteger { minimum: 1, maximum: Some(500), default: 50 },
    description: "Requested page size; the server caps it at 500.",
};

const SCHEMA_PARAMETERS: &[ParameterContract] = &[RESOURCE];
const RESOURCE_PARAMETERS: &[ParameterContract] = &[RESOURCE];
const LIST_PARAMETERS: &[ParameterContract] = &[RESOURCE, Q, SORT, OFFSET, LIMIT];
const ITEM_PARAMETERS: &[ParameterContract] = &[RESOURCE, ITEM_ID];
const ACTION_PARAMETERS: &[ParameterContract] = &[RESOURCE, ACTION];

const JSON_BODY_TYPES: &[&str] = &["application/json"];
const PATCH_BODY_TYPES: &[&str] = &["application/json", "application/merge-patch+json"];

const CREATE_BODY: RequestBodyContract = RequestBodyContract {
    description: "JSON value to store as a new resource item.",
    schema: SchemaContract::AnyJson,
    content_types: JSON_BODY_TYPES,
};
const PATCH_BODY: RequestBodyContract = RequestBodyContract {
    description: "Object-shaped JSON Merge Patch. The current item is updated with this patch.",
    schema: SchemaContract::JsonObject,
    content_types: PATCH_BODY_TYPES,
};
const ACTION_BODY: RequestBodyContract = RequestBodyContract {
    description: "JSON input passed to the provider-defined action.",
    schema: SchemaContract::AnyJson,
    content_types: JSON_BODY_TYPES,
};

/// The complete operation contract implemented by the current HTTP dispatch.
///
/// Keep this list in sync with `dispatch_operation`: it is the single source
/// for the OpenAPI paths, methods, parameters, bodies, and success responses.
pub(crate) const ENDPOINT_CONTRACT: &[EndpointContract] = &[
    EndpointContract {
        operation_id: "schema",
        method: "get",
        path: "/resources/{resource}/schema",
        summary: "Get a resource schema",
        description: "Returns field metadata and granted capabilities for a registered resource. Requires the schema capability.",
        parameters: SCHEMA_PARAMETERS,
        request_body: None,
        success_status: "200",
        success_description: "The provider resource schema.",
        success_schema: SchemaContract::ResourceSchema,
    },
    EndpointContract {
        operation_id: "list",
        method: "get",
        path: "/resources/{resource}",
        summary: "List resource items",
        description: "Lists matching items with optional text search, sorting, and offset/limit pagination. Requires the list capability.",
        parameters: LIST_PARAMETERS,
        request_body: None,
        success_status: "200",
        success_description: "A page containing items and effective pagination values.",
        success_schema: SchemaContract::ResourcePage,
    },
    EndpointContract {
        operation_id: "create",
        method: "post",
        path: "/resources/{resource}",
        summary: "Create a resource item",
        description: "Stores one JSON value and returns the created value. Requires the create capability.",
        parameters: RESOURCE_PARAMETERS,
        request_body: Some(CREATE_BODY),
        success_status: "201",
        success_description: "The created resource item.",
        success_schema: SchemaContract::AnyJson,
    },
    EndpointContract {
        operation_id: "get",
        method: "get",
        path: "/resources/{resource}/items/{id}",
        summary: "Get one resource item",
        description: "Returns one item by identifier. Requires the get capability.",
        parameters: ITEM_PARAMETERS,
        request_body: None,
        success_status: "200",
        success_description: "The requested resource item.",
        success_schema: SchemaContract::AnyJson,
    },
    EndpointContract {
        operation_id: "patch",
        method: "patch",
        path: "/resources/{resource}/items/{id}",
        summary: "Patch one resource item",
        description: "Applies an object-shaped JSON Merge Patch and returns the updated item. Requires the update capability.",
        parameters: ITEM_PARAMETERS,
        request_body: Some(PATCH_BODY),
        success_status: "200",
        success_description: "The updated resource item.",
        success_schema: SchemaContract::AnyJson,
    },
    EndpointContract {
        operation_id: "delete",
        method: "delete",
        path: "/resources/{resource}/items/{id}",
        summary: "Delete one resource item",
        description: "Deletes one item by identifier. Requires the delete capability.",
        parameters: ITEM_PARAMETERS,
        request_body: None,
        success_status: "200",
        success_description: "Confirmation that the item was deleted.",
        success_schema: SchemaContract::DeleteResponse,
    },
    EndpointContract {
        operation_id: "action",
        method: "post",
        path: "/resources/{resource}/actions/{action}",
        summary: "Invoke a provider action",
        description: "Passes JSON input to a provider-defined action and returns its JSON result. Requires the invoke capability.",
        parameters: ACTION_PARAMETERS,
        request_body: Some(ACTION_BODY),
        success_status: "200",
        success_description: "The provider-defined action result.",
        success_schema: SchemaContract::AnyJson,
    },
];

/// Builds the OpenAPI 3 document for the standalone API prefix.
pub(crate) fn openapi_document(api_prefix: &str) -> Value {
    let mut paths = Map::new();
    for endpoint in ENDPOINT_CONTRACT {
        let path = format!("{api_prefix}{}", endpoint.path);
        let operations = paths.entry(path).or_insert_with(|| Value::Object(Map::new()));
        operations
            .as_object_mut()
            .expect("OpenAPI path item is an object")
            .insert(endpoint.method.to_owned(), operation_document(endpoint));
    }

    json!({
        "openapi": OPENAPI_VERSION,
        "info": {
            "title": "ikashita Standalone API",
            "version": "1.0.0",
            "description": "OpenAPI 3 contract for the local ikashita Resource Provider HTTP adapter."
        },
        "servers": [{ "url": "/", "description": "Current local server" }],
        "tags": [{ "name": "Resources", "description": "Provider-backed JSON resources." }],
        "paths": paths,
        "components": { "schemas": schemas() }
    })
}

fn operation_document(endpoint: &EndpointContract) -> Value {
    let mut operation = Map::new();
    operation.insert("tags".to_owned(), json!(["Resources"]));
    operation.insert("summary".to_owned(), json!(endpoint.summary));
    operation.insert("description".to_owned(), json!(endpoint.description));
    operation.insert("operationId".to_owned(), json!(endpoint.operation_id));
    operation.insert(
        "parameters".to_owned(),
        Value::Array(endpoint.parameters.iter().map(parameter_document).collect()),
    );
    if let Some(body) = endpoint.request_body {
        operation.insert("requestBody".to_owned(), request_body_document(body));
    }

    let mut responses = Map::new();
    responses.insert(
        endpoint.success_status.to_owned(),
        json!({
            "description": endpoint.success_description,
            "content": { "application/json": { "schema": schema_reference(endpoint.success_schema) } }
        }),
    );
    for (status, description) in [
        ("400", "Invalid path encoding, query value, or JSON request body."),
        ("404", "The resource, item, action, or route was not found."),
        ("405", "The HTTP method or provider capability is not supported."),
        ("409", "The provider rejected the operation because it conflicts with current state."),
        ("503", "The provider is temporarily unavailable."),
        ("500", "The server or provider returned an unexpected internal failure."),
    ] {
        responses.insert(
            status.to_owned(),
            json!({
                "description": description,
                "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ErrorResponse" } } }
            }),
        );
    }
    operation.insert("responses".to_owned(), Value::Object(responses));
    Value::Object(operation)
}

fn parameter_document(parameter: &ParameterContract) -> Value {
    let (location, required, schema) = match parameter.kind {
        ParameterKind::Path => ("path", true, json!({ "type": "string" })),
        ParameterKind::QueryString => ("query", false, json!({ "type": "string" })),
        ParameterKind::QueryInteger { minimum, maximum, default } => {
            let mut schema = json!({ "type": "integer", "format": "int64", "minimum": minimum, "default": default });
            if let Some(maximum) = maximum {
                schema["maximum"] = json!(maximum);
            }
            ("query", false, schema)
        }
    };
    json!({
        "name": parameter.name,
        "in": location,
        "required": required,
        "description": parameter.description,
        "schema": schema
    })
}

fn request_body_document(body: RequestBodyContract) -> Value {
    let mut content = Map::new();
    for content_type in body.content_types {
        content
            .insert((*content_type).to_owned(), json!({ "schema": schema_reference(body.schema) }));
    }
    json!({
        "description": body.description,
        "required": true,
        "content": content
    })
}

fn schema_reference(schema: SchemaContract) -> Value {
    match schema {
        SchemaContract::AnyJson => json!({ "$ref": "#/components/schemas/JsonValue" }),
        SchemaContract::JsonObject => json!({ "$ref": "#/components/schemas/JsonObject" }),
        SchemaContract::ResourcePage => json!({ "$ref": "#/components/schemas/ResourcePage" }),
        SchemaContract::ResourceSchema => {
            json!({ "$ref": "#/components/schemas/ResourceSchema" })
        }
        SchemaContract::DeleteResponse => {
            json!({ "$ref": "#/components/schemas/DeleteResponse" })
        }
    }
}

fn schemas() -> Value {
    json!({
        "JsonValue": {},
        "JsonObject": {
            "type": "object",
            "additionalProperties": true
        },
        "ResourcePage": {
            "type": "object",
            "required": ["items", "total", "offset", "limit"],
            "properties": {
                "items": { "type": "array", "items": {} },
                "total": { "type": "integer", "format": "int64", "minimum": 0 },
                "offset": { "type": "integer", "format": "int64", "minimum": 0 },
                "limit": { "type": "integer", "format": "int64", "minimum": 1, "maximum": 500 }
            }
        },
        "ResourceSchema": {
            "type": "object",
            "required": ["name", "fields", "capabilities"],
            "properties": {
                "name": { "type": "string" },
                "fields": { "type": "array", "items": { "$ref": "#/components/schemas/FieldSchema" } },
                "capabilities": {
                    "type": "array",
                    "uniqueItems": true,
                    "items": {
                        "type": "string",
                        "enum": ["schema", "list", "get", "create", "update", "delete", "invoke"]
                    }
                }
            }
        },
        "FieldSchema": {
            "type": "object",
            "required": ["name", "field_type", "required"],
            "properties": {
                "name": { "type": "string" },
                "field_type": { "type": "string", "enum": ["text", "number", "integer", "boolean", "date", "json"] },
                "required": { "type": "boolean" },
                "enum": { "type": "array", "items": {} },
                "format": { "type": "string" }
            }
        },
        "DeleteResponse": {
            "type": "object",
            "required": ["ok"],
            "properties": { "ok": { "type": "boolean", "enum": [true] } }
        },
        "ErrorResponse": {
            "type": "object",
            "required": ["error"],
            "properties": {
                "error": {
                    "type": "object",
                    "required": ["code", "message", "fields"],
                    "properties": {
                        "code": { "type": "string", "enum": ["validation_failed", "not_found", "conflict", "capability_denied", "unavailable", "internal"] },
                        "message": { "type": "string" },
                        "fields": { "type": "object", "additionalProperties": { "type": "string" } },
                        "request_id": { "type": "string" }
                    }
                }
            }
        }
    })
}

/// Renders a local-only, dependency-free Swagger UI-compatible document view.
pub(crate) fn swagger_ui_html(api_prefix: &str) -> String {
    let document = openapi_document(api_prefix);
    let document = serde_json::to_string(&document)
        .expect("the static OpenAPI document should always serialize");
    let document = escape_script_json(&document);
    let openapi_url = format!("{api_prefix}{OPENAPI_JSON_SUFFIX}");

    format!(
        r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Swagger UI — ikashita Standalone API</title>
  <style>
    :root {{ color-scheme: light; font-family: ui-sans-serif, system-ui, sans-serif; background: #f4f6f8; color: #17202a; }}
    body {{ margin: 0; }}
    header {{ background: #1b2733; color: #fff; padding: 1.5rem max(1rem, calc((100vw - 72rem) / 2)); }}
    header h1 {{ margin: 0 0 .35rem; font-size: 1.55rem; }}
    header p {{ margin: 0; color: #c7d5e0; }}
    header a {{ color: #9ee7ff; display: inline-block; margin-top: .8rem; }}
    main {{ max-width: 72rem; margin: 1.25rem auto; padding: 0 1rem 3rem; }}
    .notice {{ background: #fff; border: 1px solid #d6dee5; border-radius: .5rem; padding: 1rem; margin-bottom: 1rem; }}
    .operation {{ background: #fff; border: 1px solid #d6dee5; border-left: .35rem solid #718096; border-radius: .45rem; margin: .8rem 0; overflow: hidden; }}
    .operation summary {{ cursor: pointer; list-style: none; padding: .9rem 1rem; display: flex; gap: .8rem; align-items: center; flex-wrap: wrap; }}
    .operation summary::-webkit-details-marker {{ display: none; }}
    .method {{ color: #fff; border-radius: .25rem; font-size: .75rem; font-weight: 800; letter-spacing: .04em; padding: .3rem .5rem; min-width: 3.6rem; text-align: center; }}
    .get {{ background: #1677b7; }} .post {{ background: #1e8449; }} .patch {{ background: #9a7d0a; }} .delete {{ background: #b03a2e; }}
    code {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }}
    .path {{ font-weight: 700; }} .operation-id {{ color: #5d6d7e; margin-left: auto; }}
    .details {{ border-top: 1px solid #e5eaee; padding: 1rem; }}
    .details h3 {{ font-size: 1rem; margin: 1rem 0 .45rem; }} .details h3:first-child {{ margin-top: 0; }}
    .details p {{ margin: .35rem 0; line-height: 1.45; }}
    table {{ border-collapse: collapse; width: 100%; margin: .35rem 0; }} th, td {{ border: 1px solid #d6dee5; padding: .5rem; text-align: left; vertical-align: top; }} th {{ background: #f4f6f8; }}
    .status {{ font-family: ui-monospace, monospace; font-weight: 700; }}
    .empty {{ color: #5d6d7e; }}
    @media (max-width: 44rem) {{ .operation-id {{ width: 100%; margin-left: 0; }} table {{ font-size: .9rem; }} }}
  </style>
</head>
<body>
  <header>
    <h1>Swagger UI — ikashita Standalone API</h1>
    <p>OpenAPI {OPENAPI_VERSION} · local, bundled, and provider-independent</p>
    <a href="{openapi_url}">Download OpenAPI JSON</a>
  </header>
  <main id="swagger-ui" aria-live="polite">
    <section class="notice">
      <strong>Resources</strong>
      <p>This Swagger UI-compatible view is rendered from the bundled OpenAPI document. It does not load scripts, styles, fonts, or data from external origins.</p>
    </section>
  </main>
  <script id="openapi-document" type="application/json">{document}</script>
  <script>
    (function () {{
      "use strict";
      var spec = JSON.parse(document.getElementById("openapi-document").textContent);
      var root = document.getElementById("swagger-ui");
      var methods = ["get", "post", "patch", "delete", "put", "options", "head"];
      function cell(value) {{ var element = document.createElement("td"); element.textContent = value || ""; return element; }}
      function section(title, rows) {{
        var heading = document.createElement("h3"); heading.textContent = title; root.currentDetails.appendChild(heading);
        if (!rows.length) {{ var empty = document.createElement("p"); empty.className = "empty"; empty.textContent = "None"; root.currentDetails.appendChild(empty); return; }}
        var table = document.createElement("table"); var header = document.createElement("tr");
        rows[0].forEach(function (value) {{ var th = document.createElement("th"); th.textContent = value; header.appendChild(th); }});
        var thead = document.createElement("thead"); thead.appendChild(header); table.appendChild(thead);
        var body = document.createElement("tbody"); rows.slice(1).forEach(function (row) {{ var tr = document.createElement("tr"); row.forEach(function (value) {{ tr.appendChild(cell(value)); }}); body.appendChild(tr); }}); table.appendChild(body); root.currentDetails.appendChild(table);
      }}
      Object.keys(spec.paths).forEach(function (path) {{
        var pathItem = spec.paths[path];
        methods.forEach(function (method) {{
          var operation = pathItem[method]; if (!operation) return;
          var card = document.createElement("details"); card.className = "operation"; card.open = true;
          var summary = document.createElement("summary"); var badge = document.createElement("span"); badge.className = "method " + method; badge.textContent = method.toUpperCase(); summary.appendChild(badge);
          var pathText = document.createElement("code"); pathText.className = "path"; pathText.textContent = path; summary.appendChild(pathText);
          var id = document.createElement("code"); id.className = "operation-id"; id.textContent = operation.operationId || ""; summary.appendChild(id); card.appendChild(summary);
          var details = document.createElement("div"); details.className = "details"; card.appendChild(details); root.appendChild(card); root.currentDetails = details;
          var description = document.createElement("p"); description.textContent = operation.description || operation.summary || ""; details.appendChild(description);
          var parameters = (operation.parameters || []).map(function (parameter) {{ return [parameter.name, parameter.in, parameter.required ? "yes" : "no", parameter.description]; }});
          section("Parameters", [["Name", "Location", "Required", "Description"]].concat(parameters));
          var requestRows = [];
          if (operation.requestBody) {{ Object.keys(operation.requestBody.content || {{}}).forEach(function (contentType) {{ requestRows.push([contentType, operation.requestBody.required ? "yes" : "no", operation.requestBody.description]); }}); }}
          section("Request body", [["Content type", "Required", "Description"]].concat(requestRows));
          var responseRows = Object.keys(operation.responses || {{}}).map(function (status) {{ return [status, operation.responses[status].description]; }});
          section("Responses", [["Status", "Description"]].concat(responseRows));
        }});
      }});
      delete root.currentDetails;
    }}());
  </script>
</body>
</html>
"##,
        OPENAPI_VERSION = OPENAPI_VERSION,
        openapi_url = openapi_url,
        document = document,
    )
}

fn escape_script_json(value: &str) -> String {
    value
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_has_all_dispatch_operations() {
        let operations: Vec<_> = ENDPOINT_CONTRACT
            .iter()
            .map(|endpoint| (endpoint.method, endpoint.operation_id))
            .collect();
        assert_eq!(
            operations,
            vec![
                ("get", "schema"),
                ("get", "list"),
                ("post", "create"),
                ("get", "get"),
                ("patch", "patch"),
                ("delete", "delete"),
                ("post", "action"),
            ]
        );
    }

    #[test]
    fn generated_ui_has_no_external_references() {
        let html = swagger_ui_html("/api/ikashita/v1");
        assert!(html.contains("Swagger UI"));
        assert!(html.contains("OpenAPI 3.0.3"));
        assert!(!html.contains("http://"));
        assert!(!html.contains("https://"));
        assert!(!html.contains("<script src"));
        assert!(!html.contains("<link rel=\"stylesheet\""));
    }

    #[test]
    fn generated_document_maps_every_contract_operation() {
        let document = openapi_document("/api/ikashita/v1");
        assert_eq!(document["openapi"], OPENAPI_VERSION);
        assert_eq!(document["paths"].as_object().map(Map::len), Some(4));

        for endpoint in ENDPOINT_CONTRACT {
            let path = format!("/api/ikashita/v1{}", endpoint.path);
            assert_eq!(
                document["paths"][path.as_str()][endpoint.method]["operationId"],
                endpoint.operation_id
            );
            assert!(
                document["paths"][path.as_str()][endpoint.method]["responses"]["400"].is_object()
            );
        }
    }
}
