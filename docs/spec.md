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
