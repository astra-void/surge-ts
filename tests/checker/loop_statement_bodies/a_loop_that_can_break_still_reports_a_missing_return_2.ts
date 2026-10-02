// `while (1)` is not folded by tsc's binder either.
export function f(): number { while (1) { } }
