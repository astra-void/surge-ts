declare const optionalAny: [a?: any];
declare const optionalUnknown: [a: string, b?: unknown];
declare const optionalNumber: [a: string, b?: number];
declare const requiredUndefined: [a: string, b: number | undefined];
declare const undefinedFirst: [a: string, b: undefined | number];
declare const frozenOptionalAny: readonly [a?: any];
declare const frozenRequired: readonly [a: string, b: number | undefined];

export const anyLength: 0 | 1 = optionalAny.length;
export const unknownLength: 1 | 2 = optionalUnknown.length;
export const numberLength: 1 | 2 = optionalNumber.length;
export const requiredLength: 2 = requiredUndefined.length;
export const undefinedFirstLength: 2 = undefinedFirst.length;
export const frozenAnyLength: 0 | 1 = frozenOptionalAny.length;
export const frozenRequiredLength: 2 = frozenRequired.length;

export function isMatching(...args: [pattern: string, value?: any]): boolean {
  if (args.length === 1) {
    const [pattern] = args;
    return pattern.length > 0;
  }
  if (args.length === 2) {
    const [pattern, value] = args;
    return pattern === value;
  }
  return false;
}
export function array(...args: [pattern?: any]): number {
  if (args.length === 0) return 0;
  return args.length === 1 ? 1 : 2;
}
export function frozenRest(...args: readonly [a: string, b?: any]): number {
  return args.length === 1 ? 1 : 2;
}
isMatching("a");
isMatching("a", 1);
array();
array(1);
frozenRest("a");

export const empty: [a?: any] = [];
export const short: [a: string, b?: number] = ["a"];
export const fromShort: [a: string, b?: unknown] = ["a"] as [string];

export const notOne: 1 = optionalAny.length;
export const notTwo: 2 = optionalUnknown.length;
export const notOneRequired: 1 = requiredUndefined.length;
export function compare() {
  if (requiredUndefined.length === 1) {
    return 1;
  }
  return 0;
}
export const shortRequired: [a: string, b: number | undefined] = ["a"] as [string];
