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
