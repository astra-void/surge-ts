declare function takes(pair: [number, string]): void;

export const tooLong: [number, string] = [1, "a", true];
export const tooShort: [number, string] = [1];
export const emptyTarget: [] = [1];
export const readonlyTarget: readonly [number] = [1, 2];
export const optionalOk: [number, string?] = [1];
export const optionalLong: [number, string?] = [1, "a", true];

takes([1, "a", true]);
takes([1]);
takes([1, 2]);

let assigned: [number, string] = [1, "a"];
assigned = [1, "a", true];

export function returns(): [number, string] {
  return [1, "a", true];
}

export const nested: { pair: [number, string] } = { pair: [1, "a", true] };
export const inArray: [number, string][] = [[1, "a", true]];
export const wrongElement: [number, string] = [1, 2];
export const bothWrong: [number, string] = [1, 2, true];
export const unionOk: [number] | [number, string] = [1, "a"];
export { assigned };
