declare const empty: [];
declare const single: [number];
declare const pair: [number, number];
declare const optionalTail: [string, number?];
declare const twoOptional: [string, number?, boolean?];
declare const frozen: readonly [string, string];
declare const open: [string, ...number[]];
declare const one: [string];

export const lengths: [0, 1, 2] = [empty.length, single.length, pair.length];
export const optionalLength: 1 | 2 = optionalTail.length;
export const twoOptionalLength: 1 | 2 | 3 = twoOptional.length;
export const frozenLength: 2 = frozen.length;
export const openLength: number = open.length;
export const first: number = single[0];
export const byPosition: { 0: string; length: 2 } = frozen;

export const tooLong: 2 = single.length;
export const notAlwaysTwo: 2 = optionalTail.length;
export const notAlwaysOne: 1 = open.length;
export function compare() {
  if (pair.length === 3) {
    return true;
  }
  switch (single.length) {
    case 2:
      return false;
  }
  return undefined;
}
export const wrongElement: { 0: number } = one;
export const missingElement: { 1: string } = one;
