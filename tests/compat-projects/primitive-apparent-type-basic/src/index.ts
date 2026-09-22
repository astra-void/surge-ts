interface Overloaded {
  pick(text: string): number;
  pick(count: number): string;
}
declare function take(value: Overloaded): void;
declare let slot: Overloaded;
declare const big: bigint;
declare const sym: symbol;

export const fromNumber: Overloaded = 3;
take(3);
take(true);
slot = big;

export const numberMembers: { toFixed(digits?: number): string } = 3;
export const booleanMembers: { valueOf(): Object } = true;
export const bigintMembers: { toString(): string } = big;
export const symbolMembers: { toString(): string } = sym;
export const emptyTargets: {}[] = [3, true, big, sym];

export const bigintObject: object = big;
export const symbolObject: object = sym;
export const bigintMissing: { missing: number } = big;
export const symbolMismatch: { description: number } = sym;
