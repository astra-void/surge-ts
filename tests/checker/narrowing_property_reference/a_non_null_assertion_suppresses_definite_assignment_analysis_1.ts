// A non-null assertion at the read site suppresses definite-assignment analysis
// for that reference, as tsc's `assumeInitialized` does.
export function f(): number {
let n: number;
return n!;
}
