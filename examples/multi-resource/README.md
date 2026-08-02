# Multi-resource project

This project demonstrates one application definition using two independent
local data providers. Both resources are read-only and are configured together in
one `resources.kdl` file.

```sh
cargo run -p ikashita-cli -- validate examples/multi-resource
cargo run -p ikashita-cli -- inspect examples/multi-resource
cargo run -p ikashita-cli -- list examples/multi-resource --resource contacts --query ada
cargo run -p ikashita-cli -- list examples/multi-resource --resource teams --sort name
cargo run -p ikashita-cli -- build examples/multi-resource
cargo run -p ikashita-cli -- test examples/multi-resource
```

The generated bundle contains the application definition only; data records are
opened by the local host and are not embedded in `dist/`.
