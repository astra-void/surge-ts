// A rest parameter written as a tuple (`(...args: [value: number, p?: string])`)
// declares positional parameters, so a callback written against it takes one
// contextual type per tuple element rather than the whole tuple in slot 0.
// @noImplicitAny: true
declare function run(f: (...args: [value: number, p?: string]) => void): void;
run((value, params) => {
const n: number = value;
const s: string | undefined = params;
void n;
void s;
});
