declare const strings: Stream<string>;
declare const literals: Stream<"a">;

// `write` is a method, so `W` relates both ways.
export const widened: Stream<string> = literals;
export const narrowed: Stream<"a"> = strings;

// The base contributes `prototype` and the construct signature.
export const proto: Stream = strings.prototype;
export const wrong: number = strings.prototype;
