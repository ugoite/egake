# `@egake/runtime`

The runtime package is a dependency-free Deno/TypeScript host boundary. Its
entrypoint is [`mod.ts`](mod.ts) (or `src/index.ts` inside this repository).

`ResourceClient` accepts only same-origin relative paths, sends a safe
`x-request-id`, uses `credentials: "same-origin"`, and never logs request bodies
or response data. `client.resource(name)` implements the documented
schema/list/get/create/update/delete/invoke `ResourceProvider` contract and
checks the advertised capability before each operation. Updates require an
object-shaped RFC 7396 merge patch; use `applyMergePatch` when an embedded
provider needs the shared merge behavior.

`parseApplication`, `renderApplication`, and `mountApplication` consume
serialized Application Profile v0.1 JSON. Rendering creates an allowlisted DOM
tree and uses `textContent`, properties, and event listeners. Application data
cannot provide HTML, scripts, remote assets, or executable expressions.

The runtime has no framework or network dependency. React and Vue applications
can use the thin adapters in `../react` and `../vue`.
