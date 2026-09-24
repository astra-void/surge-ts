declare const source: { a: number; b: { c: number; d: number }; e: number };
declare const pair: [number, [number, number], number];

export function lists() {
    var x, y = 10;
    let used = 1, unused = 2;
    return used;
}

export function patterns() {
    const [first, [second, third], fourth] = pair;
    const { a, b: { c, d }, e } = source;
    const [kept, [dropped, _ignored]] = pair;
    return kept;
}

export function exemptions() {
    const [_skip, taken] = [1, 2];
    const { a: _renamed, e } = source;
    const { _shorthand } = { _shorthand: 1 };
    const { a: removed, ...rest } = source;
    return [taken, e, rest];
}

export function oneElement() {
    const [only] = pair;
    const { a: lone } = source;
}

const moduleA = 1, moduleB = 2;
const [moduleFirst, moduleSecond] = pair;
