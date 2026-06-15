type A = Exclude<"a" | "b" | "c", "a" | "c">;
type B = Extract<"a" | "b" | "c", "a" | "c">;
type C = NonNullable<string | undefined>;

const a: A = "b";
const aBad: A = "a";

const b1: B = "a";
const b2: B = "c";
const bBad: B = "b";

const c: C = "ok";
const cBad: C = undefined;
