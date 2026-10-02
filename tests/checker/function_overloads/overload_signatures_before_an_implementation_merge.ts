// Overload signatures in front of an implementation are the same group.
function impl(v: string): number;
function impl(v: string, r: string): number;
function impl(v: string, r?: string): number { return 1; }
export const a = impl("x");
export const b = impl("x", "y");
