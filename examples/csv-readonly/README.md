# Read-only CSV list/search

This is the smallest local data workflow. The CSV intentionally has no `id`
column and the provider is not writable, so it advertises only `schema` and
`list`. Search is a case-insensitive substring search across all fields.

Run from the repository root without a server, network, or credentials:

```sh
cargo run -p ikashita-cli -- validate examples/csv-readonly
cargo run -p ikashita-cli -- list examples/csv-readonly --resource catalog --query ada --sort title
cargo run -p ikashita-cli -- list examples/csv-readonly --resource catalog --json
cargo run -p ikashita-cli -- test examples/csv-readonly
```

The list command prints one JSON object per matching row. `run` can also serve
the same project locally; no create, update, delete, or invoke capability is
required by this application.
