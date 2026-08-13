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

This package is the Egake-side data runtime only. It exports `ResourceProvider`,
`ResourceClient`, structured errors, and merge-patch helpers. It does not parse
or render UI JSON.

For UI, use [`../ikasue`](../ikasue), whose `IkaView` and Custom Elements are
the single browser UI runtime. It receives properties and emits semantic DOM
events; ResourceProvider remains here on the Egake side.
