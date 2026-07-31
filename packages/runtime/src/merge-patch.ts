import { JsonObject, JsonValue, ResourceError } from "./types.ts";

/** Returns true for JSON objects, excluding arrays and null. */
export function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Returns true when a value is composed solely of JSON data. */
export function isJsonValue(value: unknown): value is JsonValue {
  if (
    value === null || typeof value === "string" || typeof value === "boolean"
  ) return true;
  if (typeof value === "number") return Number.isFinite(value);
  if (Array.isArray(value)) return value.every(isJsonValue);
  if (isJsonObject(value)) {
    return Object.keys(value).every((key) => isJsonValue(value[key]));
  }
  return false;
}

function clone(value: JsonValue): JsonValue {
  if (Array.isArray(value)) return value.map(clone);
  if (isJsonObject(value)) {
    const result: JsonObject = Object.create(null) as JsonObject;
    for (const [key, child] of Object.entries(value)) {
      result[key] = clone(child);
    }
    return result;
  }
  return value;
}

function mergeInto(target: JsonValue, patch: JsonValue): JsonValue {
  if (!isJsonObject(patch)) return clone(patch);
  const result: JsonObject = isJsonObject(target)
    ? (clone(target) as JsonObject)
    : Object.create(null);
  for (const [key, value] of Object.entries(patch)) {
    if (value === null) {
      delete result[key];
    } else {
      result[key] = mergeInto(result[key] ?? null, value);
    }
  }
  return result;
}

/** Applies RFC 7396 merge-patch semantics without mutating either input. */
export function applyMergePatch(
  target: JsonValue,
  patch: JsonValue,
): JsonValue {
  if (!isJsonValue(target) || !isJsonValue(patch)) {
    throw new ResourceError({
      code: "validation_failed",
      message: "merge patch values must be JSON",
    });
  }
  return mergeInto(target, patch);
}

/** Validates the object-only patch required by a resource update. */
export function requireObjectPatch(
  value: JsonValue,
): asserts value is JsonObject {
  if (!isJsonObject(value) || !isJsonValue(value)) {
    throw new ResourceError({
      code: "validation_failed",
      message: "resource update patch must be a JSON object",
      fields: { patch: "expected a JSON object" },
    });
  }
}
