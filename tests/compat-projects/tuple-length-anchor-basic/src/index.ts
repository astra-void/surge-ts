declare function takesTriple(value: [number, number, string[][]]): void;
declare const holder: { pair: [string, number]; shape: { a: number; b: string } };

takesTriple([1, 2, [["x"]], 3]);
takesTriple([1, 2]);
takesTriple([1, "two", [["x"]]]);

export const tooLong: [number, number] = [1, 2, 3];
export const tooShort: [number, number, string] = [1, 2];
export const optionalTail: [number, string?] = [1];
export const nestedProperty: { pair: [number, number] } = { pair: [1, 2, 3] };

let slot: [number, number];
slot = [1, 2, 3];
holder.pair = ["a", 1, false, true];
holder.pair = ["a", 1];
holder.shape = { a: 1 };

export function returnsPair(): [string] {
  return ["a", "b"];
}
export function reads() {
  return slot;
}
