// A module-scope write reached no checker at all: the top-level statement
// parser accepted only an identifier target.
const o = { a: 1, b: "x" };
o.a = 2;
o.a = "s";
export { o };
