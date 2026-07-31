# Executable MVP decisions

This document turns the draft plan into the decisions that foundation and
future implementation work can rely on. It is intentionally narrower than a
full parser, renderer, or server specification.

## Application Profile

The MVP uses **KDL Application Profile v0.1**. An application definition starts
with KDL 2 and an `app` node whose `version` is `"0.1"`:

```kdl
/- kdl-version 2
app "contacts-admin" version="0.1" {
    // profile-defined resources, state, pages, and actions
}
```

The profile version is part of the validated application metadata and is
independent of the runtime crate version. Unknown profile versions are a
validation error; a later profile may define an explicit migration.

### Parser and typed IR

`ikashita-spec` exposes these owned entry points:

```rust
use ikashita_spec::{parse_and_validate, ApplicationDefinition};

let definition: ApplicationDefinition = parse_and_validate(source)?;
let definition = ApplicationDefinition::parse_and_validate_file("app.ui.kdl")?;
```

`parse` checks KDL syntax and profile shape. `parse_and_validate` additionally
checks all references. `ApplicationDefinition::validate()` is available for
definitions constructed or transformed by another agent. File entry points
attach the file path to diagnostics; named string entry points are useful to a
CLI that already owns the display name.

The v0.1 IR contains:

- `ApplicationProfile` (application name and profile version);
- `ResourceDefinition` (resource name, schema identifier, and a sorted set of
  `list`, `get`, `create`, `update`, `delete`, or `invoke` requirements);
- `StateDefinition` (name and an owned `serde_json::Value`);
- `PageDefinition` and a recursive `Component` tree with a closed
  `ComponentKind` set: `column`, `row`, `text`, `text-input`, `select`,
  `textarea`, `button`, `data-table`, `form`, and table `column` declarations;
- `ActionDefinition` and the known declarative steps `validate`, `upsert`,
  `refresh`, `toast`, and `invoke`; and
- `EventBinding` pairs attached to component nodes.

Known component attributes are retained in an owned JSON-valued map for a
renderer. The parser type-checks the scalar values and rejects unknown
attributes. The MVP recognizes `label`, `field`, `bind`, `action`, `resource`,
`key`, `mode`, `variant`, `align`, `gap`, and `id` where they are valid for the
specific component. `mode` is `inline`, `drawer`, or `dialog`; `variant` is
`default`, `primary`, `secondary`, `danger`, or `ghost`; `align` is `start`,
`center`, `end`, or `stretch`; and `gap` is `xs`, `sm`, `md`, `lg`, or `xl`.

State values use KDL strings, booleans, numbers, and `#null` directly. A
string beginning with a JSON object or array is decoded as that object/array
when valid. For an editor-friendly KDL form, `state "x" { value { ... } }`
is also accepted: `-` children form arrays and named children form objects.
This is the conservative v0.1 decision for the draft plan's unresolved
JSON-ish state syntax; it avoids adding a second KDL grammar while keeping
structured initial state useful.

### Diagnostics and validation

Every parser or validator failure is a `Diagnostic` with a stable
`DiagnosticCode`, `Severity`, message, and optional one-based
`SourceLocation`. `Diagnostics::render()` produces deterministic newline-
separated CLI lines in this form:

```text
error IK2104: app.ui.kdl:31:49: reference targets undeclared action 'save'
```

Diagnostics are sorted by source location, code, severity, and message. KDL
syntax diagnostics preserve the parser-provided span location; semantic
diagnostics produced directly from an owned IR have no source location.
Unknown nodes, unknown attributes, malformed scalar values, invalid enum
values, duplicate resource/state/page/action names, duplicate component IDs,
and duplicate form fields are errors. The parser never treats an unknown
component as a generic renderer extension in v0.1.

Bindings resolve to `state.<name>` or `form.<component-id>.<field>`. A `field`
attribute declares a field on the nearest containing `form`; it is invalid on
an input outside a form, and a `form.<id>.<field>` binding must resolve to one
of those declarations. The parser does not load the external schema in this
increment, so schema field/type checks remain a later validation layer.

Resource references in `data-table`, component `resource` attributes, and
declarative action steps must match a declared resource. `data-table` always
requires both `resource` and `key`. Resource `require` values are limited to
the six transport-neutral capabilities listed above; `schema` is a resource
metadata reference rather than a requestable `require` value.

Action references resolve only against top-level `action "name"` nodes in the
same `app` node. Buttons, `on "event" action="name"` pairs, and all other
component action attributes use this same exact-name lookup. There are no
implicit built-in action names in v0.1, and an external Rhai function name is
not considered declared by this parser. This conservative choice makes a
definition self-contained and interoperable with later CLI/server agents;
the draft plan's separate `actions.rhai` integration can add an explicit
declaration/import layer later without changing current resolution rules.

## Resource contract

### JSON provider boundary

The transport-neutral contract keeps the existing synchronous generic
`ResourceProvider` trait for typed and host-language adapters. Standalone
dispatch uses the object-safe JSON boundary
`JsonResourceProvider: Send + Sync`, whose methods are:

```text
schema() -> ResourceSchema
list(query: ListQuery) -> { items, total, offset, limit }
get(id) -> item or not-found
create(value: JSON object) -> item
update(id, merge_patch: JSON object) -> item
delete(id) -> empty result
invoke(action, input) -> JSON result (optional)
```

Providers are registered by name in a concurrent server registry. The HTTP
layer depends only on this boundary, not on CSV or any other storage type. A
mutex-backed adapter is available when an existing generic provider uses
mutable methods; providers with their own internal synchronization can
implement the JSON boundary directly.

### `update` uses merge-patch semantics

`update(id, patch)` accepts an object-shaped merge patch. Object members are
merged recursively, `null` removes a member, and scalar or array values replace
the existing value. Providers must validate the resulting record and return the
updated record. A provider must not interpret the patch as a complete replace.
This is the RFC 7396 object merge rule; resource update inputs are required to
be objects even though the shared merge helper also supports scalar/array
replacement for general JSON use.

### `list` uses offset/limit pagination

The list query has `q`, `sort`, `offset`, and `limit` fields. `offset` defaults
to `0`; `limit` defaults to `50` and is capped at `500`. Responses contain the
returned items, the effective `offset`, the effective `limit`, and the total
matching item count. Cursor pagination is outside the MVP contract.

The URL spelling is `q`, `sort`, `offset`, and `limit`. Sort is a comma-separated
list of field names; `-field` and `field:desc` sort descending, while an
unprefixed field or `field:asc` sorts ascending. Unknown query keys are ignored
for forward compatibility. A zero limit normalizes to one and a limit above
500 normalizes to 500.

### Errors are structured

Every provider failure is a structured error with a stable `code`, human-facing
`message`, optional field-level messages, and an optional `request_id`. The
initial code vocabulary is `validation_failed`, `not_found`, `conflict`,
`capability_denied`, `unavailable`, and `internal`. Transport adapters may add
metadata, but they must preserve the structured fields and must not reduce an
error to an untyped string.

The JSON form is `{ "error": { "code", "message", "fields", "request_id" } }`.
`fields` is an object (possibly empty), and `request_id` is always supplied by
the standalone HTTP adapter.

### CSV provider

`ikashita-csv` opens an existing regular CSV file, reads its header row, and
maps every row to a JSON object with string-valued fields. The configured
primary key defaults to `id`; writable files must contain that header, and
duplicate or empty primary keys, duplicate headers, malformed rows, path
traversal components, directories, and other non-file targets are rejected.
Read-only CSVs may omit the key, in which case only schema/list capabilities
are advertised. Search is a case-insensitive substring search over all fields;
sorting is stable lexicographic field sorting followed by offset/limit
pagination.

CSV create/update/delete operations are protected by a process-local lock
shared by providers opened for the same canonical path. A write serializes the
complete read-modify-write sequence, writes a temporary file in the CSV's
directory, flushes and syncs it, optionally copies retained backups named
`file.csv.bak.N`, and atomically renames the temporary file over the original.
CSV has a fixed column set, so a merge-patch `null` removes a JSON member and
is persisted as an empty CSV cell; the provider returns the normalized
fixed-column record. Records and values are never logged.

## Build output

`ikashita build` emits a self-contained static bundle. The output contains the
HTML entry point, the runtime assets, and the validated application bundle
needed by the runtime. It does not load runtime code from a CDN or require the
source KDL file at runtime. A host may inject Resource Providers at the
documented adapter boundary; provider data and credentials are not embedded in
the static output.

## Ugoite integration boundary

Ugoite integration is a separate adapter, not a dependency of any ikashita
crate. The adapter wraps an existing Ugoite client and maps it to the shared
Resource Contract. ikashita does not own Ugoite authentication, URLs, storage,
or data types, and the core workspace must remain buildable without a Ugoite
checkout or network access.

## Standalone HTTP adapter

`ikashita-server` exposes a testable axum router and an async `serve`/`run`
function. The default `ServerConfig` remains `127.0.0.1:8787`; binding, auth,
and external exposure are not forced by the library. The routes are:

```text
GET    /api/ikashita/v1/resources/:name/schema
GET    /api/ikashita/v1/resources/:name?q=&sort=&offset=&limit=
POST   /api/ikashita/v1/resources/:name
GET    /api/ikashita/v1/resources/:name/items/:id
PATCH  /api/ikashita/v1/resources/:name/items/:id
DELETE /api/ikashita/v1/resources/:name/items/:id
POST   /api/ikashita/v1/resources/:name/actions/:action
```

List responses are JSON objects with `items`, `total`, `offset`, and `limit`.
Create returns 201 with the created item; get/update/action return 200 with
their JSON result; delete returns 200 with `{ "ok": true }`. Unknown resources
and routes are 404. Invalid JSON, invalid query values, and provider
validation failures are 400; conflicts are 409; missing items are 404;
capability or method denials are 405; unavailable providers are 503; and
unexpected failures are 500. Every error retains the stable provider code and
includes a request ID in both the body and the `x-request-id` response header.

An incoming `x-request-id` is reused only when it is 1–128 ASCII
alphanumeric/`-_.:` characters; otherwise the server generates a
process-local deterministic ID. The HTTP layer does not log records or
request bodies. CORS is disabled by default and this increment does not add an
auth layer. An attached `StaticBundle` serves its index and named assets for
non-API GETs, with the index as the fallback for application routes.

## Host/runtime adapters

The host increment lives outside the Rust workspace in `packages/` and
`python/`. It does not add a storage provider, CLI command, or authentication
layer.

### TypeScript/Deno browser runtime

`packages/runtime/mod.ts` is the dependency-free TypeScript entrypoint. Its
`ResourceProvider` is an asynchronous-friendly JSON version of the contract:

```ts
schema(): ResourceSchema
list(query: ListQuery): ResourcePage
get(id: string): item
create(value: object): item
update(id: string, mergePatch: object): item
delete(id: string): void
invoke(action: string, input: JSON): JSON
```

Methods may return a value or a promise so an embedded host can use either a
local provider or an existing client. `ResourceClient` implements the HTTP
provider facade. It accepts only a relative API root, validates encoded
resource/item/action paths, uses `credentials: "same-origin"`, sends a safe
`x-request-id`, and converts `{error:{code,message,fields,request_id}}` into a
structured `ResourceError`. It does not accept an authorization-token option
and never logs bodies, records, cookies, or credentials. `hasCapability` and
`assertCapability` are available to injected providers and the HTTP facade;
the facade checks the provider schema before each operation.

`applyMergePatch` follows RFC 7396 and never mutates its inputs. Resource
updates require an object patch; object members merge recursively, `null`
removes a member, and arrays/scalars replace values.

`parseApplication`, `renderApplication`, and `mountApplication` consume
serialized Application Profile v0.1 JSON with this top-level shape:
`{profile:{name,version:"0.1"},resources,states,pages,actions}`. The renderer
has a closed component/attribute allowlist, creates nodes with
`document.createElement`, writes user values with `textContent` or DOM
properties, and attaches named callbacks. It does not accept HTML/script
attributes, evaluate expressions, load remote assets, or use `innerHTML`.
React and Vue adapters pass safe children/VNodes through host-provided
`createElement`/`h` primitives and do not pull either framework into this
repository.

### Python host boundary

`python/ikashita` contains a standard-library `Resource` protocol and
`ResourceBase` convenience class. `ResourceASGIApp` dispatches the same schema,
list, get, create, merge-patch update, delete, and invoke routes. It parses
`q`, comma-separated `sort`, `offset`, and `limit` (default 50, zero becomes 1,
maximum 500), preserves structured error codes/fields, validates JSON object
create/update values, and propagates a safe `x-request-id`. Sync resources and
awaitable method results are both accepted. Unexpected provider exceptions are
returned as a generic `internal` error without logging request data.

The core adapter does not know about auth. `ikashita.fastapi.create_fastapi_app`
is an optional bridge that imports FastAPI only when called; deployment hosts
may add their own middleware. Core Python tests require only the standard
library. See `examples/python-fastapi`, `examples/js-embedded`, and
`examples/ugoite-entries` for provider injection boundaries.
