type IfString<T> = T extends string ? "yes" : "no";

type A = IfString<string>;
type B = IfString<number>;

const a: A = "yes";
const aBad: A = "no";

const b: B = "no";
const bBad: B = "yes";
