type SelfKeyed = { [P in P]: string };
type SelfInUnion = { [P in "a" | P]: string };
type KeyOfSelf<K> = { [P in keyof P]: K };
type Shadowing<P extends string> = { [P in P]: number };
type Fine<K extends string> = { [P in K]: P };
const fine: Fine<"a"> = { a: "a" };
const wrong: Fine<"a"> = { a: "b" };
