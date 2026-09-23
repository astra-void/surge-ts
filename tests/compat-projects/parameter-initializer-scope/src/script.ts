// A script: its declarations are globals the binder declares before any
// signature is resolved, and the lib's globals are in scope as well.
let scriptLabel: string = "";

function readsOuter(label = scriptLabel) { var scriptLabel: number = 2; return label.length; }
function readsOuterNested(label = (inner = scriptLabel) => inner) { var scriptLabel: number = 2; return label().length; }
function readsOuterPastBodyLet(size = scriptLabel.length) { let scriptLabel = 1; return size; }
function readsLib(math = Math) { return math.floor(1.5); }

function readsItself(value = value) { return value; }
function readsLater(first = second, second = 2) { return first; }
