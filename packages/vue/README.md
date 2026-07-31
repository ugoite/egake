# `@ikashita/vue`

The Vue adapter has no Vue dependency of its own. Pass Vue's `h` function (or a
compatible host primitive) to `createVueRenderer`; use
`createVueResourceProvider` to inject a runtime provider into application code.
Serialized values become VNode text/children, never template HTML.
