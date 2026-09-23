export {};

// Rejected: a block-scoped binding read while its own declaration runs.
let counter = counter + 1;
const label = label + "";
let annotated: number = annotated;
let optional: number | undefined = optional;
for (let step = step; ; ) { break; }
for (const key in key) { }
for (const item of item) { }
for (const [first] of first) { }

export function inner() {
    let local = local + 1;
    const typed: string = typed;
    for (const entry of entry) { }
    { let nested = nested; }
    read;
    let read = 1;
}

namespace Container {
    export const member = member;
}

// A write is positional too, and the write itself is still checked.
let reassigned = (reassigned = 1);
const constant = (constant = 1);
let typedWrite: number = (typedWrite = 1);
export function writes() {
    record((pending = 3));
    let pending = 0;
}
declare function record(value: unknown): void;

// Accepted: a deferred read, and a `var`, which has no temporal dead zone.
let lazy = () => lazy;
let viaFunction = function () { return viaFunction; };
let viaMethod = { get() { return viaMethod; } };
var hoisted: number = hoisted;
for (const callback of [() => callback]) { }
export function deferredInner() {
    const recurse = () => recurse;
    for (const task of [() => task]) { }
    return recurse;
}
