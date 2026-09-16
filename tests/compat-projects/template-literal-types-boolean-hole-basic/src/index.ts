type Flag = `${boolean}`;
type Prefixed = `is-${boolean}`;
type Mixed = `v${boolean | 1}`;
type Pair = `${boolean}-${"a" | "b"}`;
type Nothing = `x${never}`;

const flagOk: Flag = "true";
const flagBad: Flag = "yes";
const prefixedOk: Prefixed = "is-false";
const prefixedBad: Prefixed = "is-maybe";
const mixedOk: Mixed = "v1";
const mixedBad: Mixed = "v2";
const pairOk: Pair = "false-b";
const pairBad: Pair = "no";
const nothing: Nothing = "x";
