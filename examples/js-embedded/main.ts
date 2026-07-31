import {
  JsonObject,
  mountApplication,
  ResourceProvider,
  SerializedApplication,
} from "../../packages/runtime/mod.ts";

/** The host supplies providers; application JSON contains no data credentials. */
export function startIkashitaHost(
  root: HTMLElement,
  application: SerializedApplication,
  providers: Readonly<Record<string, ResourceProvider<JsonObject>>>,
) {
  return mountApplication(root, application, { providers });
}

// A real host can call startIkashitaHost(document.getElementById("app")!, json, providers)
// after loading its own serialized application bundle and provider adapters.
