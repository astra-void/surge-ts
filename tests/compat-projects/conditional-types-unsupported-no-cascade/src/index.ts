// `infer` in a nested position is not modeled in this slice. The conditional
// type degrades to `unknown` instead of disappearing and cascading into
// spurious "cannot find name" / assignability diagnostics. TypeScript resolves
// the real element type; the checker falls back to `unknown`. Both agree that
// the assignments below are valid, so the fixture stays oracle-clean while
// pinning the conservative, low-cascade degrade.
type ElementOf<T> = T extends Array<infer U> ? U : T;

type StringElement = ElementOf<string[]>;
type NumberElement = ElementOf<number[]>;

const fromStrings: StringElement = "value";
const fromNumbers: NumberElement = 42;
