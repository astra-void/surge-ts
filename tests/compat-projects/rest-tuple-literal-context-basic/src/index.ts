declare function variadic(args: [string, ...number[]]): void;

export const restOk: [number, ...string[]] = [1, "a", "b"];
export const leadingOnly: [number, ...string[]] = [1];
export const trailingOk: [...string[], number] = ["a", "b", 1];
export const middleOk: [boolean, ...string[], number] = [true, "a", "b", 1];
export const callbacks: [string, ...((n: number) => string)[]] = [
  "a",
  (n) => n.toFixed(),
  (n) => n.length,
];

variadic(["a", 1, 2]);
variadic(["a"]);

export const badRest: [number, ...string[]] = [1, "a", 2];
export const badLeading: [number, ...string[]] = ["x", "a"];
export const badTrailing: [...string[], number] = ["a", "b"];
export const tooShort: [number, ...string[]] = [];
export const middleShort: [boolean, ...string[], number] = [true];
export const nestedBad: [number, ...{ a: number }[]] = [1, { a: "s" }];
export const nestedExcess: [number, ...{ a: number }[]] = [1, { a: 1, b: 2 }];

variadic(["a", "b"]);
variadic([]);
