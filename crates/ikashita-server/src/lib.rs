//! Local HTTP dispatch for JSON Resource Providers and static bundles.

pub mod bundle;
pub mod config;

use std::{
    collections::BTreeMap,
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
        if name.trim().is_empty() || name.contains('/') || name == "." || name == ".." {
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
    let path = request.uri().path().trim_start_matches(API_PREFIX).trim_matches('/').to_owned();
    let segments: Vec<&str> = path.split('/').filter(|segment| !segment.is_empty()).collect();

    let result = dispatch_operation(&state, &method, &query, &segments, request.into_body()).await;
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
    if segments.len() < 2 || segments[0] != "resources" {
        return Err(ResourceError::new(ResourceErrorKind::NotFound, "API route was not found"));
    }
    let provider = state.provider(segments[1])?;

    match segments {
        ["resources", _, "schema"] => {
            require_method(method, Method::GET)?;
            require_capability(&provider, Capability::Schema)?;
            json_response(
                StatusCode::OK,
                serde_json::to_value(provider.schema()?).map_err(internal_json)?,
            )
        }
        ["resources", _] => match *method {
            Method::GET => {
                require_capability(&provider, Capability::List)?;
                let query = ListQuery::from_query_string(query)?;
                let page = provider.list(&query)?;
                page_response(page)
            }
            Method::POST => {
                require_capability(&provider, Capability::Create)?;
                let value = parse_json_body(body).await?;
                json_response(StatusCode::CREATED, provider.create(value)?)
            }
            _ => Err(method_error()),
        },
        ["resources", _, "items", id] => match *method {
            Method::GET => {
                require_capability(&provider, Capability::Get)?;
                let value = provider.get(id)?.ok_or_else(|| {
                    ResourceError::new(ResourceErrorKind::NotFound, "resource item was not found")
                })?;
                json_response(StatusCode::OK, value)
            }
            Method::PATCH => {
                require_capability(&provider, Capability::Update)?;
                let patch = parse_json_body(body).await?;
                require_object_patch(&patch)?;
                json_response(StatusCode::OK, provider.update(id, patch)?)
            }
            Method::DELETE => {
                require_capability(&provider, Capability::Delete)?;
                provider.delete(id)?;
                json_response(StatusCode::OK, json!({ "ok": true }))
            }
            _ => Err(method_error()),
        },
        ["resources", _, "actions", action] => {
            require_method(method, Method::POST)?;
            require_capability(&provider, Capability::Invoke)?;
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
    capability: Capability,
) -> ResourceResult<()> {
    let schema = provider.schema()?;
    if schema.capabilities.contains(&capability) {
        Ok(())
    } else {
        Err(ResourceError::new(
            ResourceErrorKind::CapabilityDenied,
            format!("resource does not expose the {} capability", capability_name(capability)),
        ))
    }
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
    error.request_id = Some(request_id.to_owned());
    let body = serde_json::to_vec(&json!({ "error": error.to_json() })).unwrap_or_else(|_| {
        b"{\"error\":{\"code\":\"internal\",\"message\":\"internal error\"}}".to_vec()
    });
    Response::builder()
        .status(capability_status(&error))
        .header(header::CONTENT_TYPE, "application/json")
        .header(REQUEST_ID_HEADER, request_id)
        .body(Body::from(body))
        .unwrap_or_else(|_| {
            Response::new(Body::from(b"{\"error\":{\"code\":\"internal\"}}".to_vec()))
        })
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
        return error_response(
            ResourceError::new(ResourceErrorKind::NotFound, "route was not found"),
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
    if let Some(contents) = bundle.assets().get(requested) {
        let content_type = asset_content_type(requested);
        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(contents.clone()))
            .unwrap_or_else(|_| Response::new(Body::empty()));
        return response_with_request_id(response, &request_id);
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
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "wasm" => "application/wasm",
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
            let mut schema = ResourceSchema::new("memory");
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
        assert_eq!(schema["name"], "memory");

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
}
