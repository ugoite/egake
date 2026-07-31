# Third-party notices

ikashita is distributed under the MIT License; see [`LICENSE`](LICENSE).

The foundation workspace currently has no third-party Rust, JavaScript, or
WASM runtime dependencies. Its crates use the Rust standard library only.
`Cargo.lock` is committed so that future dependencies can be reviewed as part
of a reproducible change.

When a dependency is added, its license expression, copyright notice, and any
required license text must be recorded here (and included in every relevant
distribution). `deny.toml` provides the baseline license and source policy;
the dependency lockfile is the authoritative list for each release.
