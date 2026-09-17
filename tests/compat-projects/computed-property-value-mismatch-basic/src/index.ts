const key = "alpha";
declare const declaredKey: "beta";
const KEYS = { gamma: "gamma" } as const;

// A value under a computed, non-literal name has its own message.
const fromConst: { alpha: number } = { [key]: "one" };
const fromDeclared: { beta: number } = { [declaredKey]: "one" };
const fromMember: { gamma: number } = { [KEYS.gamma]: "one" };
const signed: { [-1]: number } = { [-1]: "one" };

function takes(value: { alpha: number }): void {}
takes({ [key]: "one" });

// A literal name — quoted, numeric, or a plain template — does not.
const quoted: { alpha: number } = { ["alpha"]: "one" };
const numeric: { 1: number } = { [1]: "one" };
const template: { alpha: number } = { [`alpha`]: "one" };

// A nested literal is elaborated to its own property instead.
const nested: { alpha: { depth: number } } = { [key]: { depth: "deep" } };
