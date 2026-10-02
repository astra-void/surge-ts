// Two *implementations* are a duplicate declaration, not an overload group.
function dup(v: string): string { return v; }
function dup(v: string): string { return v; }
export const a = dup("x");
