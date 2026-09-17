declare const text: string;
declare const kind: "a" | "b";
declare const mixed: string | number;

export const toNumber = text as number;
export const angle = <number>text;
export const literal = 1 as string;
export const sibling = kind as "c";
export const member = kind as "a";
export const unrelatedLiterals = "x" as "y";
export const inArithmetic = (text as number) + 1;
export const toBoolean = mixed as boolean;
export const narrowed = mixed as string;
export function inside() {
  return text as number;
}
