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

### `update` uses merge-patch semantics

`update(id, patch)` accepts an object-shaped merge patch. Object members are
merged recursively, `null` removes a member, and scalar or array values replace
the existing value. Providers must validate the resulting record and return the
updated record. A provider must not interpret the patch as a complete replace.

### `list` uses offset/limit pagination

The list query has `q`, `sort`, `offset`, and `limit` fields. `offset` defaults
to `0`; `limit` defaults to `50` and is capped at `500`. Responses contain the
returned items, the effective `offset`, the effective `limit`, and the total
matching item count. Cursor pagination is outside the MVP contract.

### Errors are structured

Every provider failure is a structured error with a stable `code`, human-facing
`message`, optional field-level messages, and an optional `request_id`. The
initial code vocabulary is `validation_failed`, `not_found`, `conflict`,
`capability_denied`, `unavailable`, and `internal`. Transport adapters may add
metadata, but they must preserve the structured fields and must not reduce an
error to an untyped string.

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
