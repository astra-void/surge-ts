export {};
// TS2393, TS7006, TS2355
function dup() {} // TS2393
function dup() {}

function implicit_any(x) { // TS7006
    return x;
}

function no_return(): number { // TS2355
}
