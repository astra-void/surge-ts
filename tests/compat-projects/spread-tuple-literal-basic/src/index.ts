declare const strs: string[];
declare const nums: number[];
declare const pair: [string, string];
declare function takes(tuple: [number, ...string[]]): void;

type Kind = { kind: "a" | "b" };

export const restFromArray: [number, ...string[]] = [1, ...strs];
export const restFromTuple: [number, ...string[]] = [1, ...pair];
export const restFromLiteral: [number, ...string[]] = [1, ...["a", "b"]];
export const fixedFromTuple: [number, string, string] = [1, ...pair];
export const fixedWhole: [string, string] = [...pair];
export const fixedTrailing: [string, string, number] = [...pair, 1];
export const literalSlot: ["x", string, string] = ["x", ...pair];
export const contextualObject: [Kind, ...string[]] = [{ kind: "a" }, ...strs];
export const contextualCallback: [(n: number) => string, ...string[]] = [
  (n) => n.toFixed(),
  ...strs,
];
export const plainArray: string[] = [...pair, ...strs];

takes([1, ...strs]);

export const wrongRest: [number, ...string[]] = [1, ...nums];
export const missingLeading: [number, ...string[]] = [...strs];
export const openIntoFixed: [number, string, string] = [1, ...strs];
export const tooLong: [number, string] = [1, ...pair];
export const wrongTrailing: [string, string, number] = [...pair, "x"];
export const nestedBeforeSpread: [Kind, string, string] = [{ kind: "z" }, ...pair];

takes([1, ...nums]);
