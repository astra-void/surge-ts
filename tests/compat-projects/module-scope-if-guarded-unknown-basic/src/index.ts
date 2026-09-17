declare const value: unknown;
declare function isRecord(input: unknown): input is Record<string, unknown>;

// A guarded `unknown` is not the `unknown` keyword in a module-scope branch,
// exactly as in a function body.
if (typeof value === "object" && value !== null && "id" in value) {
  value.id;
}
if (value && typeof value === "object" && "name" in value) {
  value.name;
}
if (isRecord(value)) {
  value.anything;
}

// Unguarded, it still is.
if (Math.random() > 0.5) {
  value.id;
}
