type Fn = (n: number) => { ok: boolean };
declare const anyValue: any;

// No annotation, so tsc infers the return type from the returns: one `any`
// collapses the union and the whole-signature relation passes. Nothing here.
export const absorbed: Fn = (n) => {
  if (n > 0) {
    return anyValue;
  }
  return { ok: 'nope' };
};

// The `any` inside a ternary branch collapses it just the same.
export const absorbedInBranch: Fn = (n) => {
  return n > 0 ? anyValue : { ok: 'nope' };
};

// No `any` anywhere: the mismatch is still reported. Written on one line
// because tsc anchors this on the declaration and surge on the return — the
// spans differ, the line does not.
export const reported: Fn = (n) => { return { ok: 'nope' }; };

// An annotation restores per-return checking.
export const annotated: Fn = (n): { ok: boolean } => {
  return { ok: 'nope' };
};

// Errors that are not return-assignability survive the collapse.
export const otherErrorsSurvive: Fn = (n) => {
  if (n > 0) {
    return anyValue;
  }
  return { ok: notDeclaredAnywhere };
};

// A missing property is a return-value verdict too, so the collapse covers it.
export const absorbedMissingProperty: Fn = (n) => { if (n > 0) { return anyValue; } return {}; };

// A property-level `any` does NOT collapse the union — the return's own type is
// an object, not `any` — so this still reports.
type Boxed = (n: number) => { data: { id: string }; error: undefined };
export const propertyAnyStillReports: Boxed = (n) => { let d: any; return { data: d, error: 1 }; };

// An `any` return in a NESTED function belongs to that function's own body.
export const nestedAnyDoesNotLeak: Fn = (n) => { const inner: Fn = (m) => { return anyValue; }; return { ok: 'nope' }; };

// The collapse belongs to the body that owns it. A nested function with its own
// signature keeps its returns checked, even inside a collapsed outer body.
export const nestedAnnotatedArrowStillChecked: Fn = (n) => {
  const inner = (): { ok: boolean } => { return { ok: 'nope' }; };
  void inner;
  if (n > 0) { return anyValue; }
  return { ok: true };
};

// Probing the returned value for `any` must not itself report. Method shorthand
// in a returned object literal has a contextual signature, and the inference
// pass routes it through the checking entry with no expected type.
type Handlers = { onItem(value: string[]): void };
export const handlers: () => Handlers = () => {
  return {
    onItem(value) {
      void value.length;
    },
  };
};
