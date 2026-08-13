# Ikasue Web ABI

The machine-readable UI contract is `ikasue-web/1`. `view.schema.json` defines
the JSON-safe `IkaView` tree; `data-grid.schema.json` defines the controlled
DataGrid properties and semantic DOM event details. The browser ABI is the Web
Platform: Custom Element properties in, composed DOM events out.

Egake-only resource, state, and action metadata belongs in the application
bundle's `bindings`, never in `IkaView.props`. There is no MessagePort or
framework adapter in the core v1 contract.
