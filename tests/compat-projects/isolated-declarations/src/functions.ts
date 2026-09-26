declare function random(): number;
export function noReturn() {}
export function untypedParameter(p): void {}
export function badDefault(p = 1 + 1): void {}
export function literalReturn() { return 1; }
export function twoReturns(b: boolean) { if (b) return 1; return 2; }
export const arrow = () => random();
export const arrowOk = (): number => random();
type N = number;
export const callback = (p: N = 1, v: number): void => {};
export function overloaded(a: string): string;
export function overloaded(a: number): number;
export function overloaded(a: any) { return a; }
export function expando(): void {}
expando.extra = 1;
expando.extra = 2;
