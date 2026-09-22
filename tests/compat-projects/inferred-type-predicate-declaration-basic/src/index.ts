function isString(x: string | number) { return typeof x === "string"; }
const isStringArrow = (x: string | number) => typeof x === "string";
declare let sn: string | number;

function byDeclaration() {
    if (isString(sn)) { const a: string = sn; const wrong: number = sn; } else { const b: number = sn; }
}
function byArrow() {
    if (isStringArrow(sn)) { const a: string = sn; } else { const b: number = sn; const wrong: string = sn; }
}

type Shape = { kind: "c"; r: number } | { kind: "s"; w: number };
function isCircle(shape: Shape) { return shape.kind === "c"; }
function discriminated(shape: Shape) {
    if (isCircle(shape)) { shape.r; shape.w; } else { shape.w; shape.r; }
}

function isDefined(v: string | undefined | null) { return v != null; }
function nullish(v: string | undefined | null) {
    if (isDefined(v)) { v.length; } else { const n: number = v; }
}

declare const items: (string | undefined)[];
function isPresent(v: string | undefined) { return v !== undefined; }
const present: string[] = items.filter(isPresent);
const notNarrowed: string[] = items.filter(truthy);
function truthy(v: string | undefined) { return !!v; }

function secondParameter(a: string | number, b: string | number) { return typeof b === "string"; }
function second(p: string | number, q: string | number) {
    if (secondParameter(p, q)) { const x: string = q; const y: string = p; }
}

function withStatementBefore(x: string | number) { console.log(x); return typeof x === "string"; }
function before(p: string | number) { if (withStatementBefore(p)) { const x: string = p; } }

function flaky(x: string | number) { return typeof x === "string" && Math.random() > 0.5; }
function notIff(p: string | number) { if (flaky(p)) { const x: string = p; } }

function annotated(x: string | number): boolean { return typeof x === "string"; }
function writtenBoolean(p: string | number) { if (annotated(p)) { const x: string = p; } }

function nested() {
    function inner(x: string | number) { return typeof x === "number"; }
    return (p: string | number) => { if (inner(p)) { const n: number = p; } };
}
export {};
