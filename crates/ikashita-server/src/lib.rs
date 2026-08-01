//! Local HTTP dispatch for JSON Resource Providers and static bundles.

pub mod bundle;
pub mod config;

use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderValue, Method, Request, Response, StatusCode, header},
    routing::any,
};
use ikashita_resource::{
    Capability, JsonResourceProvider, ListQuery, ResourceError, ResourceErrorKind, ResourcePage,
    ResourceResult, require_object_patch,
};
use serde_json::{Value, json};
use tokio::net::TcpListener;

pub use bundle::StaticBundle;
pub use config::ServerConfig;

const API_PREFIX: &str = "/api/ikashita/v1";
const MAX_JSON_BODY: usize = 2 * 1024 * 1024;
const MAX_QUERY_BYTES: usize = 16 * 1024;
const REQUEST_ID_HEADER: &str = "x-request-id";

/// Shared state used by the testable router and local server.
#[derive(Clone)]
pub struct ServerState {
    providers: Arc<RwLock<BTreeMap<String, Arc<dyn JsonResourceProvider>>>>,
    bundle: Option<Arc<StaticBundle>>,
    request_counter: Arc<AtomicU64>,
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerState {
    /// Creates an empty provider registry without an attached static bundle.
    #[must_use]
    pub fn new() -> Self {
        Self {
            providers: Arc::new(RwLock::new(BTreeMap::new())),
            bundle: None,
            request_counter: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Attaches a static bundle to this state.
    #[must_use]
    pub fn with_bundle(mut self, bundle: StaticBundle) -> Self {
        self.bundle = Some(Arc::new(bundle));
        self
    }

    /// Registers a named provider.
    pub fn register_provider<P>(&self, name: impl Into<String>, provider: P) -> ResourceResult<()>
    where
        P: JsonResourceProvider + 'static,
    {
        self.register_arc(name, Arc::new(provider))
    }

    /// Registers an already shared provider.
    pub fn register_arc(
        &self,
        name: impl Into<String>,
        provider: Arc<dyn JsonResourceProvider>,
    ) -> ResourceResult<()> {
        let name = name.into();
        if !is_safe_route_name(&name) {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "resource name is invalid",
            ));
        }
        let mut providers = self.providers.write().map_err(|_| {
            ResourceError::new(ResourceErrorKind::Internal, "provider registry is poisoned")
        })?;
        if providers.contains_key(&name) {
            return Err(ResourceError::new(
                ResourceErrorKind::Conflict,
                "resource name is already registered",
            ));
        }
        providers.insert(name, provider);
        Ok(())
    }

    /// Returns the current registered provider for a name.
    pub fn provider(&self, name: &str) -> ResourceResult<Arc<dyn JsonResourceProvider>> {
        self.providers
            .read()
            .map_err(|_| {
                ResourceError::new(ResourceErrorKind::Internal, "provider registry is poisoned")
            })?
            .get(name)
            .cloned()
            .ok_or_else(|| {
                ResourceError::new(ResourceErrorKind::NotFound, "resource was not found")
            })
    }

    fn request_id(&self, request: &Request<Body>) -> String {
        request
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .filter(|value| is_safe_request_id(value))
            .map(str::to_owned)
            .unwrap_or_else(|| {
                let number = self.request_counter.fetch_add(1, Ordering::Relaxed) + 1;
                format!("req-{}-{number}", std::process::id())
            })
    }
}

/// Builds a testable axum router from shared server state.
pub fn router(state: Arc<ServerState>) -> Router {
    Router::new()
        .route(&format!("{API_PREFIX}/{{*path}}"), any(api_dispatch))
        .route(API_PREFIX, any(api_dispatch))
        .with_state(state.clone())
        .fallback(static_dispatch)
        .with_state(state)
}

/// Builds a router; this descriptive alias is convenient for embedding hosts.
pub fn build_router(state: Arc<ServerState>) -> Router {
    router(state)
}

/// Serves the configured router on the configured address.
pub async fn serve(config: ServerConfig, state: Arc<ServerState>) -> io::Result<()> {
    let state = match config.bundle().cloned() {
        Some(bundle) => Arc::new((*state).clone().with_bundle(bundle)),
        None => state,
    };
    let listener = TcpListener::bind(config.address()).await?;
    axum::serve(listener, router(state)).await.map_err(io::Error::other)
}

/// Runs the local HTTP server; this is an alias for serve for CLI call sites.
pub async fn run(config: ServerConfig, state: Arc<ServerState>) -> io::Result<()> {
    serve(config, state).await
}

async fn api_dispatch(
    axum::extract::State(state): axum::extract::State<Arc<ServerState>>,
    request: Request<Body>,
) -> Response<Body> {
    let request_id = state.request_id(&request);
    let method = request.method().clone();
    let query = request.uri().query().unwrap_or_default().to_owned();
    let path = request.uri().path().trim_start_matches(API_PREFIX).trim_matches('/');
    let segments = match decode_route_segments(path, true) {
        Ok(segments) => segments,
        Err(error) => return error_response(error, &request_id),
    };
    let segment_refs: Vec<&str> = segments.iter().map(String::as_str).collect();

    let result =
        dispatch_operation(&state, &method, &query, &segment_refs, request.into_body()).await;
    match result {
        Ok(response) => response_with_request_id(response, &request_id),
        Err(error) => error_response(error, &request_id),
    }
}

async fn dispatch_operation(
    state: &ServerState,
    method: &Method,
    query: &str,
    segments: &[&str],
    body: Body,
) -> ResourceResult<Response<Body>> {
    if query.len() > MAX_QUERY_BYTES {
        return Err(ResourceError::new(
            ResourceErrorKind::Validation,
            "request query is too large",
        ));
    }
    if segments.len() < 2 || segments[0] != "resources" {
        return Err(ResourceError::new(ResourceErrorKind::NotFound, "API route was not found"));
    }
    let provider = state.provider(segments[1])?;

    match segments {
        ["resources", _, "schema"] => {
            require_method(method, Method::GET)?;
            let schema = checked_schema(&provider, segments[1])?;
            if !schema.capabilities.contains(&Capability::Schema) {
                return Err(capability_error(Capability::Schema));
            }
            json_response(StatusCode::OK, serde_json::to_value(schema).map_err(internal_json)?)
        }
        ["resources", _] => match *method {
            Method::GET => {
                require_capability(&provider, segments[1], Capability::List)?;
                let query = ListQuery::from_query_string(query)?;
                let page = provider.list(&query)?;
                page_response(page)
            }
            Method::POST => {
                require_capability(&provider, segments[1], Capability::Create)?;
                let value = parse_json_body(body).await?;
                json_response(StatusCode::CREATED, provider.create(value)?)
            }
            _ => Err(method_error()),
        },
        ["resources", _, "items", id] => match *method {
            Method::GET => {
                require_capability(&provider, segments[1], Capability::Get)?;
                let value = provider.get(id)?.ok_or_else(|| {
                    ResourceError::new(ResourceErrorKind::NotFound, "resource item was not found")
                })?;
                json_response(StatusCode::OK, value)
            }
            Method::PATCH => {
                require_capability(&provider, segments[1], Capability::Update)?;
                let patch = parse_json_body(body).await?;
                require_object_patch(&patch)?;
                json_response(StatusCode::OK, provider.update(id, patch)?)
            }
            Method::DELETE => {
                require_capability(&provider, segments[1], Capability::Delete)?;
                provider.delete(id)?;
                json_response(StatusCode::OK, json!({ "ok": true }))
            }
            _ => Err(method_error()),
        },
        ["resources", _, "actions", action] => {
            require_method(method, Method::POST)?;
            require_capability(&provider, segments[1], Capability::Invoke)?;
            let input = parse_json_body(body).await?;
            json_response(StatusCode::OK, provider.invoke(action, input)?)
        }
        _ => Err(ResourceError::new(ResourceErrorKind::NotFound, "API route was not found")),
    }
}

async fn parse_json_body(body: Body) -> ResourceResult<Value> {
    let bytes = to_bytes(body, MAX_JSON_BODY).await.map_err(|_| {
        ResourceError::new(
            ResourceErrorKind::Validation,
            "request JSON body is missing, too large, or unreadable",
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|_| {
        ResourceError::new(ResourceErrorKind::Validation, "request body is not valid JSON")
            .with_field("json", "expected one JSON value")
    })
}

fn require_method(actual: &Method, expected: Method) -> ResourceResult<()> {
    if *actual == expected { Ok(()) } else { Err(method_error()) }
}

fn method_error() -> ResourceError {
    ResourceError::new(
        ResourceErrorKind::CapabilityDenied,
        "HTTP method is not supported for this route",
    )
}

fn require_capability(
    provider: &Arc<dyn JsonResourceProvider>,
    resource_name: &str,
    capability: Capability,
) -> ResourceResult<()> {
    let schema = checked_schema(provider, resource_name)?;
    if schema.capabilities.contains(&capability) {
        Ok(())
    } else {
        Err(capability_error(capability))
    }
}

fn checked_schema(
    provider: &Arc<dyn JsonResourceProvider>,
    resource_name: &str,
) -> ResourceResult<ikashita_resource::ResourceSchema> {
    let schema = provider.schema()?;
    if schema.name != resource_name || schema.name.trim().is_empty() {
        return Err(ResourceError::new(
            ResourceErrorKind::Internal,
            "provider schema does not match the registered resource",
        ));
    }
    let mut fields = BTreeSet::new();
    if schema
        .fields
        .iter()
        .any(|field| field.name.trim().is_empty() || !fields.insert(field.name.as_str()))
    {
        return Err(ResourceError::new(
            ResourceErrorKind::Internal,
            "provider returned an invalid resource schema",
        ));
    }
    Ok(schema)
}

fn capability_error(capability: Capability) -> ResourceError {
    ResourceError::new(
        ResourceErrorKind::CapabilityDenied,
        format!("resource does not expose the {} capability", capability_name(capability)),
    )
}

fn capability_status(error: &ResourceError) -> StatusCode {
    match error.kind {
        ResourceErrorKind::Validation => StatusCode::BAD_REQUEST,
        ResourceErrorKind::NotFound => StatusCode::NOT_FOUND,
        ResourceErrorKind::Conflict => StatusCode::CONFLICT,
        ResourceErrorKind::CapabilityDenied => StatusCode::METHOD_NOT_ALLOWED,
        ResourceErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        ResourceErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn json_response(status: StatusCode, value: Value) -> ResourceResult<Response<Body>> {
    let body = serde_json::to_vec(&value).map_err(internal_json)?;
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .map_err(|_| {
            ResourceError::new(ResourceErrorKind::Internal, "could not build HTTP response")
        })
}

fn page_response(page: ResourcePage<Value>) -> ResourceResult<Response<Body>> {
    json_response(
        StatusCode::OK,
        json!({
            "items": page.items,
            "total": page.total,
            "offset": page.offset,
            "limit": page.limit,
        }),
    )
}

fn error_response(error: ResourceError, request_id: &str) -> Response<Body> {
    let mut error = error;
    if matches!(error.kind, ResourceErrorKind::Internal | ResourceErrorKind::Unavailable) {
        error.message = match error.kind {
            ResourceErrorKind::Internal => "internal server error".to_owned(),
            ResourceErrorKind::Unavailable => "resource provider is unavailable".to_owned(),
            _ => unreachable!(),
        };
        error.fields.clear();
    }
    error.request_id = Some(request_id.to_owned());
    let body = serde_json::to_vec(&json!({ "error": error.to_json() })).unwrap_or_else(|_| {
        format!(
            "{{\"error\":{{\"code\":\"internal\",\"message\":\"internal server error\",\"request_id\":\"{request_id}\"}}}}"
        )
        .into_bytes()
    });
    Response::builder()
        .status(capability_status(&error))
        .header(header::CONTENT_TYPE, "application/json")
        .header(REQUEST_ID_HEADER, request_id)
        .body(Body::from(body.clone()))
        .unwrap_or_else(|_| Response::new(Body::from(body)))
}

fn response_with_request_id(mut response: Response<Body>, request_id: &str) -> Response<Body> {
    if let Ok(value) = HeaderValue::from_str(request_id) {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }
    response
}

async fn static_dispatch(
    axum::extract::State(state): axum::extract::State<Arc<ServerState>>,
    request: Request<Body>,
) -> Response<Body> {
    let request_id = state.request_id(&request);
    if request.method() != Method::GET {
        return error_response(method_error(), &request_id);
    }
    if request.uri().query().is_some_and(|query| query.len() > MAX_QUERY_BYTES) {
        return error_response(
            ResourceError::new(ResourceErrorKind::Validation, "request query is too large"),
            &request_id,
        );
    }
    let Some(bundle) = &state.bundle else {
        return error_response(
            ResourceError::new(ResourceErrorKind::NotFound, "static bundle is not attached"),
            &request_id,
        );
    };
    let requested = request.uri().path().trim_start_matches('/');
    if requested.is_empty() {
        return response_with_request_id(
            text_response(StatusCode::OK, bundle.index_html(), "text/html; charset=utf-8"),
            &request_id,
        );
    }
    let requested_segments = match decode_route_segments(requested, false) {
        Ok(segments) => segments,
        Err(error) => return error_response(error, &request_id),
    };
    let requested = requested_segments.join("/");
    if let Some(contents) = bundle.assets().get(&requested) {
        let content_type = asset_content_type(&requested);
        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(contents.clone()))
            .unwrap_or_else(|_| Response::new(Body::empty()));
        return response_with_request_id(response, &request_id);
    }
    if requested.rsplit('/').next().is_some_and(|name| name.contains('.')) {
        return error_response(
            ResourceError::new(ResourceErrorKind::NotFound, "static asset was not found"),
            &request_id,
        );
    }
    response_with_request_id(
        text_response(StatusCode::OK, bundle.index_html(), "text/html; charset=utf-8"),
        &request_id,
    )
}

fn text_response(status: StatusCode, text: &str, content_type: &str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(text.to_owned()))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

fn asset_content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "wasm" => "application/wasm",
        "ico" => "image/x-icon",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn internal_json(_: serde_json::Error) -> ResourceError {
    ResourceError::new(ResourceErrorKind::Internal, "could not encode JSON response")
}

fn capability_name(capability: Capability) -> &'static str {
    match capability {
        Capability::Schema => "schema",
        Capability::List => "list",
        Capability::Get => "get",
        Capability::Create => "create",
        Capability::Update => "update",
        Capability::Delete => "delete",
        Capability::Invoke => "invoke",
    }
}

fn is_safe_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
}

fn is_safe_route_name(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value != "."
        && value != ".."
        && !value.chars().any(|character| character.is_control() || matches!(character, '/' | '\\'))
}

fn decode_route_segments(path: &str, require_resource_name: bool) -> ResourceResult<Vec<String>> {
    if path.is_empty() {
        return Ok(Vec::new());
    }
    if path.split('/').any(str::is_empty) {
        return Err(ResourceError::new(ResourceErrorKind::NotFound, "route was not found"));
    }
    let mut segments = Vec::new();
    for raw_segment in path.split('/') {
        if !has_valid_percent_encoding(raw_segment) {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "URL path contains invalid encoding",
            ));
        }
        let segment = percent_encoding::percent_decode_str(raw_segment)
            .decode_utf8()
            .map_err(|_| {
                ResourceError::new(ResourceErrorKind::Validation, "URL path is not valid UTF-8")
            })?
            .into_owned();
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.chars().any(char::is_control)
            || segment.contains('\\')
            || (!require_resource_name && segment.contains('/'))
        {
            return Err(ResourceError::new(ResourceErrorKind::NotFound, "route was not found"));
        }
        if segments.is_empty() && require_resource_name && segment != "resources" {
            return Err(ResourceError::new(ResourceErrorKind::NotFound, "API route was not found"));
        }
        segments.push(segment);
    }
    if require_resource_name
        && segments.first().map(String::as_str) == Some("resources")
        && segments.get(1).is_some_and(|name| !is_safe_route_name(name))
    {
        return Err(ResourceError::new(ResourceErrorKind::NotFound, "API route was not found"));
    }
    Ok(segments)
}

fn has_valid_percent_encoding(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return false;
            }
            index += 3;
        } else {
            index += 1;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;
    use axum::http::Request;
    use ikashita_resource::{
        FieldSchema, FieldType, JsonResourceProvider, ListQuery, ResourceErrorKind, ResourcePage,
        ResourceSchema,
    };
    use serde_json::json;
    use tower::ServiceExt;

    use super::*;

    #[derive(Default)]
    struct MemoryProvider {
        items: std::sync::Mutex<BTreeMap<String, Value>>,
    }

    impl MemoryProvider {
        fn new() -> Self {
            let mut items = BTreeMap::new();
            items.insert("1".to_owned(), json!({"id":"1", "name":"Ada"}));
            Self { items: std::sync::Mutex::new(items) }
        }

        fn schema_value() -> ResourceSchema {
            let mut schema = ResourceSchema::new("contacts");
            schema.push_field(FieldSchema::new("id", FieldType::Text).required());
            schema.push_field(FieldSchema::new("name", FieldType::Text));
            for capability in [
                Capability::Schema,
                Capability::List,
                Capability::Get,
                Capability::Create,
                Capability::Update,
                Capability::Delete,
                Capability::Invoke,
            ] {
                schema.grant(capability);
            }
            schema
        }
    }

    impl JsonResourceProvider for MemoryProvider {
        fn schema(&self) -> ResourceResult<ResourceSchema> {
            Ok(Self::schema_value())
        }

        fn list(&self, query: &ListQuery) -> ResourceResult<ResourcePage<Value>> {
            let values: Vec<Value> = self.items.lock().expect("lock").values().cloned().collect();
            let total = values.len() as u64;
            let items =
                values.into_iter().skip(query.offset as usize).take(query.limit as usize).collect();
            Ok(ResourcePage::new(items, total, query.offset, query.limit))
        }

        fn get(&self, id: &str) -> ResourceResult<Option<Value>> {
            Ok(self.items.lock().expect("lock").get(id).cloned())
        }

        fn create(&self, value: Value) -> ResourceResult<Value> {
            let id = value["id"].as_str().unwrap_or_default().to_owned();
            let mut items = self.items.lock().expect("lock");
            if items.contains_key(&id) {
                return Err(ResourceError::new(ResourceErrorKind::Conflict, "duplicate"));
            }
            items.insert(id, value.clone());
            Ok(value)
        }

        fn update(&self, id: &str, patch: Value) -> ResourceResult<Value> {
            let mut items = self.items.lock().expect("lock");
            let item = items
                .get_mut(id)
                .ok_or_else(|| ResourceError::new(ResourceErrorKind::NotFound, "missing"))?;
            *item = ikashita_resource::apply_merge_patch(item.clone(), &patch)?;
            Ok(item.clone())
        }

        fn delete(&self, id: &str) -> ResourceResult<()> {
            self.items
                .lock()
                .expect("lock")
                .remove(id)
                .map(|_| ())
                .ok_or_else(|| ResourceError::new(ResourceErrorKind::NotFound, "missing"))
        }

        fn invoke(&self, action: &str, input: Value) -> ResourceResult<Value> {
            if action == "echo" {
                Ok(input)
            } else {
                Err(ResourceError::new(ResourceErrorKind::NotFound, "action was not found"))
            }
        }
    }

    async fn request(app: Router, request: Request<Body>) -> (StatusCode, Value) {
        let response = app.oneshot(request).await.expect("response");
        let status = response.status();
        let body = to_bytes(response.into_body(), MAX_JSON_BODY).await.expect("body");
        (status, serde_json::from_slice(&body).expect("JSON"))
    }

    async fn raw_request(app: Router, request: Request<Body>) -> (StatusCode, String) {
        let response = app.oneshot(request).await.expect("response");
        let status = response.status();
        let body = to_bytes(response.into_body(), MAX_JSON_BODY).await.expect("body");
        (status, String::from_utf8(body.to_vec()).expect("UTF-8"))
    }

    fn app() -> Router {
        let state = Arc::new(ServerState::new());
        state.register_provider("contacts", MemoryProvider::new()).expect("register");
        router(state)
    }

    #[test]
    fn registration_rejects_duplicates_and_path_ambiguous_names() {
        let state = ServerState::new();
        state.register_provider("contacts", MemoryProvider::new()).expect("register");
        let duplicate = state
            .register_provider("contacts", MemoryProvider::new())
            .expect_err("duplicate registration");
        assert_eq!(duplicate.kind, ResourceErrorKind::Conflict);
        let traversal = state
            .register_provider("../contacts", MemoryProvider::new())
            .expect_err("unsafe registration");
        assert_eq!(traversal.kind, ResourceErrorKind::Validation);
    }

    #[tokio::test]
    async fn serves_all_resource_endpoint_paths() {
        let app = app();
        let (status, schema) = request(
            app.clone(),
            Request::builder()
                .method(Method::GET)
                .uri("/api/ikashita/v1/resources/contacts/schema")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(schema["name"], "contacts");

        let (status, page) = request(
            app.clone(),
            Request::builder()
                .uri("/api/ikashita/v1/resources/contacts?q=Ada&offset=0&limit=10")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(page["items"][0]["name"], "Ada");

        let (status, created) = request(
            app.clone(),
            Request::builder()
                .method(Method::POST)
                .uri("/api/ikashita/v1/resources/contacts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"id":"2","name":"Grace"}"#))
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created["id"], "2");

        let (status, item) = request(
            app.clone(),
            Request::builder()
                .uri("/api/ikashita/v1/resources/contacts/items/2")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(item["name"], "Grace");

        let (status, updated) = request(
            app.clone(),
            Request::builder()
                .method(Method::PATCH)
                .uri("/api/ikashita/v1/resources/contacts/items/2")
                .body(Body::from(r#"{"name":"Grace Hopper"}"#))
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["name"], "Grace Hopper");

        let (status, deleted) = request(
            app.clone(),
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/ikashita/v1/resources/contacts/items/2")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(deleted["ok"], true);

        let (status, action) = request(
            app,
            Request::builder()
                .method(Method::POST)
                .uri("/api/ikashita/v1/resources/contacts/actions/echo")
                .body(Body::from(r#"{"ok":true}"#))
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(action, json!({"ok": true}));
    }

    #[tokio::test]
    async fn returns_structured_failures_and_serves_static_bundle() {
        let state = Arc::new(ServerState::new().with_bundle({
            let mut bundle = StaticBundle::new("<html>app</html>");
            bundle.insert_asset("app.js", b"console.log(1)".to_vec());
            bundle
        }));
        state.register_provider("contacts", MemoryProvider::new()).expect("register");
        let app = router(state);

        let (status, error) = request(
            app.clone(),
            Request::builder()
                .uri("/api/ikashita/v1/resources/missing")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(error["error"]["code"], "not_found");
        assert!(error["error"]["request_id"].as_str().is_some());

        let (status, error) = request(
            app.clone(),
            Request::builder()
                .method(Method::POST)
                .uri("/api/ikashita/v1/resources/contacts")
                .body(Body::from("not json"))
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(error["error"]["code"], "validation_failed");

        let (status, index) = raw_request(
            app.clone(),
            Request::builder().uri("/").body(Body::empty()).expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(index, "<html>app</html>");

        let (status, asset) = raw_request(
            app,
            Request::builder().uri("/app.js").body(Body::empty()).expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(asset, "console.log(1)");
    }

    #[tokio::test]
    async fn decodes_encoded_ids_but_rejects_encoded_route_traversal() {
        let app = app();
        let (status, item) = request(
            app.clone(),
            Request::builder()
                .uri("/api/ikashita/v1/resources/contacts/items/%31")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(item["id"], "1");

        let (status, error) = request(
            app,
            Request::builder()
                .uri("/api/ikashita/v1/resources/contacts/items/%2e%2e")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(error["error"]["code"], "not_found");

        let (status, error) = request(
            super::tests::app(),
            Request::builder()
                .uri("/api/ikashita/v1/resources/contacts/items/%ZZ")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(error["error"]["code"], "validation_failed");
    }

    #[tokio::test]
    async fn bounds_queries_and_does_not_fallback_missing_assets_to_html() {
        let state = Arc::new(ServerState::new().with_bundle(StaticBundle::new("<html>app</html>")));
        state.register_provider("contacts", MemoryProvider::new()).expect("register");
        let app = router(state);
        let (status, index) = raw_request(
            app.clone(),
            Request::builder().uri("/").body(Body::empty()).expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(index, "<html>app</html>");

        let oversized_query = "q=".to_owned() + &"x".repeat(MAX_QUERY_BYTES);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/ikashita/v1/resources/contacts?{oversized_query}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response = app
            .oneshot(Request::builder().uri("/missing.js").body(Body::empty()).expect("request"))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    }

    #[tokio::test]
    async fn rejects_provider_schema_name_mismatch_without_leaking_provider_details() {
        let state = Arc::new(ServerState::new());
        state.register_provider("contacts", MismatchedSchemaProvider).expect("register");
        let (status, error) = request(
            router(state),
            Request::builder()
                .uri("/api/ikashita/v1/resources/contacts/schema")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error["error"]["code"], "internal");
        assert_eq!(error["error"]["message"], "internal server error");
    }

    struct MismatchedSchemaProvider;

    impl JsonResourceProvider for MismatchedSchemaProvider {
        fn schema(&self) -> ResourceResult<ResourceSchema> {
            Ok(ResourceSchema::new("other-resource"))
        }

        fn list(&self, _query: &ListQuery) -> ResourceResult<ResourcePage<Value>> {
            Ok(ResourcePage::new(Vec::new(), 0, 0, 50))
        }

        fn get(&self, _id: &str) -> ResourceResult<Option<Value>> {
            Ok(None)
        }

        fn create(&self, value: Value) -> ResourceResult<Value> {
            Ok(value)
        }

        fn update(&self, _id: &str, patch: Value) -> ResourceResult<Value> {
            Ok(patch)
        }

        fn delete(&self, _id: &str) -> ResourceResult<()> {
            Ok(())
        }
    }
}
