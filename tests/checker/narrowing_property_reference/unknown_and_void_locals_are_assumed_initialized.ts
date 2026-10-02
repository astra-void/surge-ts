// A declared `unknown`/`void` local is assumed initialized (tsc's
// `AnyOrUnknown | Void` gate), so an unassigned read is not TS2454.
export function f(): unknown {
let e: unknown;
return e;
}
export function g(): void {
let v: void;
return v;
}
