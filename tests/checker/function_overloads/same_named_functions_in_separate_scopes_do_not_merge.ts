// The group is per name, and a same-named function in another scope is a
// different binding: merging across them would silence a real mismatch.
export function outerA() {
function shared(v: string): string { return v; }
return shared(1);
}
export function outerB() {
function shared(v: number): number { return v; }
return shared("a");
}
