// A script's functions are all declared before any signature is resolved, so
// a signature reads its own function and the ones declared after it.
function selfParameter(x: typeof selfParameter, y: number) { return y; }
function overloaded(n: typeof overloaded): string;
function overloaded(n: typeof declaredLater): string;
function overloaded() { return ""; }
function declaredLater(n: typeof declaredLater): number;
function declaredLater(n: typeof overloaded): number;
function declaredLater() { return 1; }
function selfReturn(): typeof selfReturn { return selfReturn; }
function selfDefault(x = selfDefault) { return x; }
function callsLater(x = computedLater()) { return x; }
function computedLater() { return 1; }

function wrongCall(x = computedLater()) { const s: string = x; return s; }
