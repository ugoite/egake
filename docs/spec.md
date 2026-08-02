---
title: 実行可能なMVP仕様
description: ikashitaのResource Contract、KDL Application Profile、CLI、ホスト境界の正本。
sidebar:
  label: 実行可能なMVP仕様
---

This page is the executable contract. Beginner-oriented explanations live in
the guide pages; when an explanation and this page disagree, this page and the
implementation take precedence.

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

#### Schema metadata decision

`ResourceSchema.fields` keeps the existing `name`, `field_type`, and `required`
members and may now include two additive members: `enum` (the JSON Schema enum
values) and `format` (`email`, `date`, or `date-time`). JSON Schema `string`
maps to `text`, `number` to `number`, `integer` to `integer`, `boolean` to
`boolean`, and `object`/`array`/`null` to `json`; a date format uses the
existing `date` field type while retaining the exact format. The members are
omitted when absent, so existing provider responses and host adapters remain
valid. The CLI derives this metadata once from the supported external schema,
passes it into configured providers, and includes the same field declarations
in `app.bundle.json` without embedding records or credentials. CSV columns not
declared by the external schema remain backward-compatible text fields; a
configured CSV must contain every schema-declared property so generated forms
cannot advertise fields the fixed-column provider cannot persist.

The browser uses the metadata only for declarative affordances and local
feedback: enum values populate `select` options, formats choose native
email/date/datetime inputs, and required/enum/format constraints produce
field-aware validation errors. Providers remain authoritative and validate
configured CSV writes as well; an absent optional CSV cell is treated as an
absent JSON property rather than an invalid empty value.
Resource names are non-empty, trimmed, control-free single path segments;
`.`, `..`, `/`, and `\\` are invalid. Provider schemas returned through an
HTTP adapter must use the registered resource name and unique, non-empty field
names. Registration is exact-name and duplicate registration is a conflict; it
never replaces an existing provider.

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
500 normalizes to 500. Encoded query strings are limited to 16 KiB; malformed
encoding and invalid pagination values are validation failures.

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

### Local data provider

`ikashita-data` opens an existing regular CSV or Parquet file and maps each
row to a JSON object. The format is inferred from the file extension or can be
set explicitly in the resource configuration. CSV fields are strings and CSV
resources may be writable; Parquet fields preserve their supported Arrow types
and Parquet resources are read-only. The configured primary key defaults to
`id`; duplicate or empty primary keys, duplicate fields, malformed rows, path
traversal components, directories, and other non-file targets are rejected.
Read-only resources may omit the key, in which case only schema/list
capabilities are advertised. Search is a case-insensitive substring search
over all fields; sorting is stable lexicographic field sorting followed by
offset/limit pagination.

CSV create/update/delete operations are protected by a process-local lock
shared by providers opened for the same canonical path. A write serializes the
complete read-modify-write sequence, writes a temporary file in the CSV's
directory, flushes and syncs it, optionally copies retained backups named
`file.csv.bak.N`, and atomically renames the temporary file over the original.
The containing directory is synced after replacement. Backup destinations must
be regular files or removable symlinks, so a backup cannot redirect a write to
another target. Failed writes remove only their temporary file and leave the
previous primary and completed backup generations recoverable.
CSV has a fixed column set, so a merge-patch `null` removes a JSON member and
is persisted as an empty CSV cell; the provider returns the normalized
fixed-column record. Records and values are never logged or echoed in storage
error details.

## Build output

`ikashita build` emits a self-contained static bundle by default. The output
contains `index.html`, `runtime.js`, `runtime.css`, and `app.bundle.json`; it
does not load runtime code from a CDN or require the source KDL file at
runtime. A host may inject Resource Providers at the documented adapter
boundary; provider data and credentials are not embedded in the static output.

`ikashita build --format single-html` (also `--single-html`) selects the
standalone artifact format. `--output dist` writes one file at
`dist/index.html`; an output path ending in `.html`, such as
`--output dist/app.html`, is written directly. The document contains exactly
one generated HTML artifact: the runtime CSS is in a `<style>` block, the
runtime JavaScript is in one executable `<script>` block, and the validated
application metadata is in a `<script type="application/json">` block. The
metadata block is read with `textContent` and `JSON.parse`; it is not executable.

The standalone document's CSP uses `default-src 'none'`, same-origin API
`connect-src`, and SHA-256 hashes for the exact inline runtime/style contents.
The JSON serializer replaces `<`, `>`, `&`, U+2028, and U+2029 with JSON
unicode escapes, so `</script>`-like application strings cannot break out of
the data block. The browser runtime first consumes that block and falls back
to the directory build's same-origin application asset only when the block is
absent. The normal `run`/`dev` server continues to attach and serve the
directory-style `StaticBundle`; `StaticBundle` also exposes whether a host-held
bundle has no assets and is therefore a single document.

When the single-html target shares a directory with a previous directory build,
the CLI removes only the four known generated asset names before writing the
document. It refuses to replace a symlink or other non-file at those names.

## CLI project and runtime increment

The Rust `ikashita` binary resolves a project directory from a positional
directory, `--project DIR`, or `.`. It requires `ikashita.toml` with an
`[app]` table. `app.definition` defaults to `app.ui.kdl`; `app.name`, when
present, must match the KDL `app` name. The commands are `new`, `validate`,
`inspect`, `build`, `run`, `dev`, `test`, and `list`. `list` opens one
configured data provider directly and supports the contract's search, sort,
offset, and limit values without starting a server. Validation diagnostics retain
the spec diagnostic codes and are stable in text or `--json` form. Schema
diagnostics use the CLI range `IK3002` and data/config diagnostics use
`IK3000`–`IK3003`.

The CLI loads resource provider configuration from one of these equivalent
conventions. `resources.kdl`, when present, is the selected source; otherwise
the TOML tables are used. Supplying both sources is a hard error, so there is
no merge or per-field override precedence:

```toml
[resources.contacts]
path = "data/contacts.csv"
key = "id"
writable = true
backup_count = 2
```

```kdl
/- kdl-version 2
resources {
    resource "contacts" path="data/contacts.csv" key="id" writable=#true backup-count=2
}
```

CSV paths, schema paths, definition paths, and build output paths are
project-relative and reject absolute paths and `..` components. Existing path
components are canonicalized, so symlinks that resolve outside the project are
also rejected, including symlinked bundle asset directories. The CLI checks
the MVP JSON Schema subset (`object`, required string names, property types,
enum, and `email`/`date`/`date-time` formats), derives provider field metadata
from it, and checks configured data rows without printing record values. A
resource must expose every capability named
by its application definition before `run`/`dev` starts.

`run` and `dev` construct `DataResourceProvider` instances, attach the generated
`StaticBundle` to `ikashita-server::ServerState`, and serve the documented
same-origin `/api/ikashita/v1` routes. They default to `127.0.0.1:8787`; a
non-loopback host requires `--allow-external` and emits an explicit warning.
The server has no CORS layer by default. `dev` currently means a local
development server with an in-memory generated bundle; file watching is
intentionally outside this increment.

The generated browser runtime uses only local JavaScript/CSS and DOM APIs. It
renders the validated component tree, searches and refreshes tables, opens
create/edit forms, uses provider/bundle schema metadata for select and native
date/email controls, saves with POST/PATCH, deletes after `confirm`, invokes a
declared provider action with JSON input, and shows structured provider
failures as field-aware errors/toasts. It uses
`textContent`/DOM construction and does not use `eval`, arbitrary HTML
injection, CDN assets, remote URLs, or embedded resource records.

`new` creates a working contacts CRUD fixture with `ikashita.toml`,
`app.ui.kdl`, `resources.kdl`, a JSON schema, `data/contacts.csv`, and an
`actions.rhai` documentation placeholder. The placeholder is never executed;
the CLI does not provide an OS command or Rhai execution boundary.

## Framework host adapters

The framework packages are deliberately thin. They import only the shared
runtime types and accept host primitives, so a project owns its Solid/Svelte/
React/Vue version and dependency graph. All adapters parse the Application
Profile before rendering and pass strings as text/children or ordinary
attributes; none uses `innerHTML`, template HTML, or script injection.

For Solid, import `createSolidRenderer` and pass `createElement`,
`createComponent`, `insert`, plus host callbacks for attributes and events:

```ts
import { createElement, createComponent, insert } from "solid-js/web";
import { createSolidRenderer } from "./packages/solid/mod.ts";

const renderer = createSolidRenderer({
  createElement,
  createComponent,
  insert,
  setAttribute: (node, name, value) => node.setAttribute(name, value),
  listen: (node, event, listener) => node.addEventListener(event, listener),
});
const applicationTree = renderer(applicationJson);
```

For Svelte, import `createSvelteRenderer`, implement the small element/text/
append/clear/listener boundary, and mount it from an action or wrapper:

```ts
import { createSvelteRenderer } from "./packages/svelte/mod.ts";

const renderer = createSvelteRenderer({
  createElement: (type) => document.createElement(type),
  createText: (value) => document.createTextNode(value),
  append: (parent, child) => parent.appendChild(child),
  clear: (parent) => parent.replaceChildren(),
  setAttribute: (node, name, value) => node.setAttribute(name, value),
  listen: (node, event, listener) => {
    node.addEventListener(event, listener);
    return () => node.removeEventListener(event, listener);
  },
});
const mounted = renderer(document.querySelector("#app"), applicationJson);
```

Use `createSolidResourceProvider` or `createSvelteResourceProvider` with a
runtime `ResourceClient` when an action needs a host provider. The Svelte mount
has `update` and `destroy`; the Solid result is handed to the host's normal
`render` lifecycle.

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
process-local deterministic ID. JSON bodies are capped at 2 MiB and encoded
list queries at 16 KiB. API path segments are percent-decoded exactly once;
malformed encodings, control characters, reserved dot segments, and unsafe
resource names are rejected, while encoded item IDs remain one ID segment.
Unsupported methods and denied capabilities are 405. Unstructured adapter
failures map 400/404/405/409/5xx to the corresponding contract categories.
Internal and unavailable errors are returned with generic messages and without
storage details. The HTTP layer does not log records or request bodies. CORS is
disabled by default and this increment does not add an auth layer. An attached
`StaticBundle` serves its index and named assets for non-API GETs, with the
index as the fallback only for extensionless application routes; missing
extension-bearing assets are 404. Asset paths cannot contain traversal or
backslashes, and served assets receive content types derived from their safe
extensions.

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
resource/item/action paths and bounded queries, uses
`credentials: "same-origin"`, sends a safe `x-request-id`, validates returned schema and
page/result shapes, and converts `{error:{code,message,fields,request_id}}` into
a structured `ResourceError`. Unsafe response request IDs are discarded. It
does not accept an authorization-token option and never logs bodies, records,
cookies, or credentials. `hasCapability` and `assertCapability` are available
to injected providers and the HTTP facade; the facade checks the provider
schema before each operation.

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
returned as a generic `internal` error without logging request data. It rejects
malformed/oversized query and body input, decodes path segments once, validates
provider schema/result shapes, and does not echo internal provider messages.

The core adapter does not know about auth. `ikashita.fastapi.create_fastapi_app`
is an optional bridge that imports FastAPI only when called; deployment hosts
may add their own middleware. Core Python tests require only the standard
library. See `examples/python-fastapi`, `examples/js-embedded`, and
`examples/ugoite-entries` for provider injection boundaries.

## Documentation and adapter availability

The repository-level `docs/` directory is the single source of truth for
published documentation. `docsite/src/content/docs` is a symlink to that
directory, and `docsite/src/content.config.ts` loads it with Starlight's
`docsLoader()`. `astro.config.mjs` also marks the canonical directory as
processed Markdown. The check task verifies the symlink target and rejects a
copied content tree, keeping the GitHub-rendered Markdown, editor view, and
built site on the same files.

The host adapter surface currently shipped in this checkout is:

| Host | Shipped entry point | Contract |
| --- | --- | --- |
| Browser / JavaScript | `packages/runtime/mod.ts` | `ResourceProvider`, `ResourceClient`, `mountApplication` |
| Solid | `packages/solid/mod.ts` | `createSolidRenderer`, `createSolidResourceProvider` |
| Svelte | `packages/svelte/mod.ts` | `createSvelteRenderer`, `createSvelteResourceProvider` |
| React | `packages/react/mod.ts` | `createReactRenderer`, `createReactResourceProvider` |
| Vue | `packages/vue/mod.ts` | `createVueRenderer`, `createVueResourceProvider` |
| Python ASGI | `python/ikashita` | `ResourceASGIApp`, `ResourceBase` |
| Ugoite | `examples/ugoite-entries/adapter.ts` | example adapter around a host-owned client |

Solid and Svelte adapters follow the same serialized-application and
`ResourceProvider` contracts as the other framework packages.
