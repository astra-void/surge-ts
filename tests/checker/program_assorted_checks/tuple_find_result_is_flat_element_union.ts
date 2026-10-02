// `find` on an `as const` tuple yields a flat `element | undefined` union that
// is assignable to (and truthy-narrows against) its written equivalent.
const A = ["x", "y"] as const;
const found: "x" | "y" | undefined = A.find((d) => d.length > 0);
function pick(): "x" | "y" {
const inner = A.find((d) => d.length > 0);
if (inner) return inner;
return "x";
}
