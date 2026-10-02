// @noPropertyAccessFromIndexSignature: true
// `T & { url: string }` degrades the bare type parameter, and the merge keeps
// the survivor open with a synthetic `any` string index so the dropped
// operand's members are not reported as excess properties. That synthetic
// index is not a declared index signature, so TS4111 must not fire on it.
function use<T extends { path?: string }>(o: T & { url: string }) { return o.path; }
