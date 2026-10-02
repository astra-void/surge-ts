// tsc only runs the missing-return check when the function's end point is
// reachable, and its binder folds only the `true`/`false` *keywords*.
export function f(): number { for (;;) { } }
