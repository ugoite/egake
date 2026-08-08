---
title: Executable MVP specification
description: The source of truth for egake's Resource Contract, KDL Application Profile, CLI, and host boundaries.
sidebar:
  label: Executable MVP specification
---

<!-- i18n-sync: id=spec digest=b70f34fcb192659870e29206083200a2d7a2cb7c3e7bbf4ab01e4d8c0f258b4a -->

This page is the executable contract. Beginner-oriented explanations live in the guide pages; when an explanation and this page disagree, this page and the implementation take precedence.

The specification records the decisions that foundation and future implementation work can rely on. It is intentionally narrower than a full parser, renderer, or server specification.

## Application Profile

The MVP uses **KDL Application Profile v0.1**. An application definition starts with KDL 2 and an `app` node whose `version` is `"0.1"`:

```kdl
/- kdl-version 2
app "contacts-admin" version="0.1" {
    // profile-defined resources, state, pages, and actions
}
```

The profile version is part of validated application metadata and is independent of the runtime crate version. Unknown profile versions are validation errors; a later profile may define an explicit migration.

### Parser and typed IR

`egake-spec` exposes owned entry points such as:

```rust
use egake_spec::{parse_and_validate, ApplicationDefinition};

let definition: ApplicationDefinition = parse_and_validate(source)?;
let definition = ApplicationDefinition::parse_and_validate_file("app.ui.kdl")?;
```

`parse` checks KDL syntax and profile shape. `parse_and_validate` additionally checks references, while `ApplicationDefinition::validate()` validates an IR constructed by another agent. File entry points attach the file path to diagnostics; named string entry points are useful when a CLI already owns the display name.

The v0.1 IR contains an application profile, resource definitions with sorted transport-neutral requirements, JSON-valued state, recursive pages and components, declared actions, and event bindings. The closed component set is `column`, `row`, `text`, `text-input`, `select`, `textarea`, `button`, `data-table`, `form`, and table `column` declarations. Known attributes include `label`, `field`, `bind`, `action`, `resource`, `key`, `mode`, `variant`, `align`, `gap`, and `id` where valid.

State values accept KDL strings, booleans, numbers, and `#null`. A string beginning with a valid JSON object or array is decoded as structured state. The editor-friendly `state "x" { value { ... } }` form is also accepted: `-` children form arrays and named children form objects.

### Diagnostics and validation

Every parser or validator failure is a structured `Diagnostic` with a stable code, severity, message, and optional one-based source location. Rendering is deterministic, for example:

```text
error IK2104: app.ui.kdl:31:49: reference targets undeclared action 'save'
```

Diagnostics sort by source location, code, severity, and message. Unknown nodes and attributes, malformed scalars, invalid enum values, duplicate names or component IDs, and duplicate form fields are errors. The parser never treats an unknown component as a renderer extension in v0.1.

Bindings resolve to `state.<name>` or `form.<component-id>.<field>`. A `field` belongs to the nearest containing form, and a `form.<id>.<field>` binding must name one of that form's fields. Resource references in tables, components, and action steps must match a declared resource. `data-table` requires both `resource` and `key`. Action references resolve only against top-level `action "name"` nodes in the same app; there are no implicit built-in action names.

## Resource contract

### JSON provider boundary

The transport-neutral contract keeps the synchronous generic `ResourceProvider` trait for typed and host-language adapters. Standalone dispatch uses the object-safe JSON boundary `JsonResourceProvider: Send + Sync`:

```text
schema() -> ResourceSchema
list(query: ListQuery) -> { items, total, offset, limit }
get(id) -> item or not-found
create(value: JSON object) -> item
update(id, merge_patch: JSON object) -> item
delete(id) -> empty result
invoke(action, input) -> JSON result (optional)
```

Providers are registered by name in a concurrent server registry. A mutex-backed adapter is available for mutable generic providers; providers with their own synchronization can implement the JSON boundary directly.

#### Schema metadata decision

`ResourceSchema.fields` keeps `name`, `field_type`, and `required`, and may add `enum` and `format` (`email`, `date`, or `date-time`). JSON Schema `string`, `number`, `integer`, `boolean`, and `object`/`array`/`null` map to the corresponding runtime field types. Absent additive members are omitted so existing provider responses and adapters remain valid.

The CLI derives metadata once from the supported external schema, passes it to providers, and includes the same field declarations in `app.bundle.json` without records or credentials. CSV columns not declared by the external schema remain backward-compatible text fields, while a configured CSV must contain every schema-declared property. The browser uses metadata for select options, native date/email controls, and local required/enum/format feedback; providers remain authoritative.

Resource names are trimmed, non-empty, control-free single path segments. `.`, `..`, `/`, and `\\` are invalid. HTTP provider schemas must use the registered name and unique non-empty fields. Registration is exact-name and duplicate registration is a conflict; it never replaces an existing provider.

### `update` uses merge-patch semantics

`update(id, patch)` accepts an object-shaped RFC 7396 merge patch. Object members merge recursively, `null` removes a member, and scalar or array values replace the existing value. Providers validate the resulting record and return it; they must not interpret the patch as a complete replacement.

### `list` uses offset/limit pagination

The list query has `q`, `sort`, `offset`, and `limit`. `offset` defaults to `0`; `limit` defaults to `50` and is capped at `500`. Responses include the returned items, effective offset and limit, and total matching count. Cursor pagination is outside the MVP.

Sort is a comma-separated list of field names; `-field` and `field:desc` descend, while `field` and `field:asc` ascend. Unknown query keys are ignored for forward compatibility. A zero limit becomes one, values above 500 become 500, encoded queries are limited to 16 KiB, and malformed encoding or invalid pagination values are validation failures.

### Errors are structured

Provider failures have a stable `code`, human-facing `message`, optional field-level messages, and optional `request_id`. The initial codes are `validation_failed`, `not_found`, `conflict`, `capability_denied`, `unavailable`, and `internal`. The JSON form is `{ "error": { "code", "message", "fields", "request_id" } }`; `fields` is an object and the standalone HTTP adapter always supplies a request ID.

### Local data provider

`egake-data` opens an existing regular CSV or Parquet file and maps each row to a JSON object. The format is inferred from the file extension or set explicitly in resource configuration. CSV fields are strings and CSV resources may be writable; Parquet fields preserve supported Arrow types and Parquet resources are read-only. The primary key defaults to `id`; duplicate or empty keys, duplicate fields, malformed rows, path traversal, directories, and other non-file targets are rejected. Read-only resources may omit the key and then advertise only schema/list capabilities. Search is a case-insensitive substring over all fields; sorting is stable lexicographic field sorting followed by offset/limit pagination.

Writes use a process-local lock shared by providers opened for the same canonical path. The provider serializes read-modify-write, writes and syncs a temporary file, optionally retains regular-file backups, atomically replaces the original, and syncs the containing directory. A failed write removes only its temporary file. CSV has a fixed column set: merge-patch `null` persists as an empty cell, and records and values are never echoed in storage errors.

## Build output

`egake build` emits a self-contained static bundle by default: `index.html`, `runtime.js`, `runtime.css`, and `app.bundle.json`. It does not load runtime code from a CDN or require the source KDL at runtime. Provider data and credentials are not embedded.

`--format single-html` (also `--single-html`) writes one HTML document. CSS and JavaScript are inline, validated metadata is in a non-executable JSON script block, and the document has no external runtime/application assets. Its CSP uses `default-src 'none'`, same-origin API `connect-src`, and hashes for the exact inline contents. Script-sensitive characters are escaped in serialized application data.

## CLI project and runtime increment

The Rust `egake` binary resolves a project from a positional directory, `--project DIR`, or `.`. It requires `egake.toml` with an `[app]` table; `app.definition` defaults to `app.ui.kdl`, and `app.name`, when present, must match the KDL app name. Commands are `new`, `validate`, `inspect`, `build`, `run`, `dev`, `test`, and `list`.

Resource configuration selects `resources.kdl` when present, otherwise the TOML tables. Supplying both is a hard error. Definition, schema, data, and output paths are project-relative and reject absolute paths and `..`; symlinks resolving outside the project are also rejected. The CLI checks the supported JSON Schema subset, derives provider metadata, validates data rows without printing their values, and ensures every declared capability is provided before `run` or `dev` starts.

`run` and `dev` construct data providers, attach a generated `StaticBundle` to the server state, and serve same-origin `/api/egake/v1` routes on loopback by default. A non-loopback host requires `--allow-external` and an explicit warning. `dev` is a local server with an in-memory bundle; watching is outside this increment.

## Framework host adapters

Framework adapters are thin translation layers. The host supplies render primitives and the adapter does not add the framework as a dependency. React and Vue packages are shipped in this checkout. Solid and Svelte use the generic runtime boundary unless a host supplies its own thin adapter. Each adapter must preserve serialized strings as text, recurse safely through children, and keep provider ownership outside rendering.

## Ugoite integration boundary

Ugoite is a host-owned client, not an egake dependency. The adapter maps `schema`, `list`, `get`, `create`, `update`, `delete`, and optional `invoke` to the Resource Contract. Authentication, URLs, cookies, retries, and data types remain in the host. The core builds without a Ugoite checkout.

## Standalone HTTP adapter

The HTTP adapter registers providers by exact resource name and exposes the versioned same-origin routes under `/api/egake/v1`. It provides schema and list/get/create/update/delete operations, plus provider-defined `POST /resources/:name/actions/:action` invoke. It validates path segments, request IDs, query encoding, JSON bodies, capabilities, and limits before dispatch. Errors remain structured and include a request ID; the adapter does not add a default CORS layer.

The standalone server also exposes a provider-independent OpenAPI 3 document at
`GET /api/egake/v1/openapi.json` and a local Swagger-compatible viewer at
`GET /api/egake/v1/swagger`. The viewer embeds its CSS, JavaScript, and
OpenAPI document, and never loads a CDN, external asset, font, or remote data.

## Host/runtime adapters

### TypeScript/Deno browser runtime

The browser runtime uses local JavaScript, CSS, and DOM APIs. It renders the validated component tree, searches and refreshes tables, opens forms, uses schema metadata for select and native date/email controls, saves with POST/PATCH, deletes only after `confirm`, and invokes declared provider actions. A host injects providers through `mountApplication`; runtime code never receives credentials or evaluates arbitrary code.

### Python host boundary

`ResourceASGIApp` provides the standard-library ASGI boundary and the optional FastAPI bridge adds framework integration without changing the contract. The host owns authentication and provider construction. Routes, request IDs, body/query limits, method handling, structured errors, and action invoke match the standalone HTTP adapter. Optional dependencies are not installed by the core tests.

## Documentation and adapter availability

The repository-level `docs/` tree is the documentation source of truth. The Starlight site reads it directly and checks Japanese/English topic pairs with synchronization markers. Guide pages explain the implementation without redefining it; the executable specification and checked-in examples decide what is actually supported.

Only adapters and commands present in this checkout may be documented as shipped. A framework or service boundary without an implementation is described as a host-owned generic boundary, with no invented package, import, URL, or credential flow.
