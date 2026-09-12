function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

declare const empty: {};
declare const fromUnknown: unknown;

export function predicateOnEmpty() {
  if (isRecord(empty)) {
    return empty["key"];
  }
  return undefined;
}

export function predicateOnUnknown() {
  if (isRecord(fromUnknown)) {
    return fromUnknown["key"];
  }
  return undefined;
}

export function presenceOnEmpty() {
  if ("message" in empty) {
    return String(empty.message);
  }
  return "";
}

// A prototype member is present on every object, so the test proves nothing and
// must not shadow the real member.
export function prototypeMember() {
  if ("toString" in empty) {
    return empty.toString();
  }
  return "";
}

// Absence proves nothing about a type that never declared the key, so the else
// branch is unchanged and the read there is still an error.
export function elseBranchUnchanged() {
  if ("message" in empty) {
    return "";
  }
  return empty.message;
}

// The guard's effect ends with the guard.
export function outsideTheGuard() {
  if ("message" in empty) {
    void empty.message;
  }
  return empty.message;
}
