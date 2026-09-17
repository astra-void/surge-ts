export function f(): string { return 1; }
export const g = function (): string { return 1; };
export class C { m(): string { return 1; } get x(): number { return "a"; } }
export function obj(): { a: string } { return { a: 1 }; }
export function cond(c: boolean): string { return c ? 1 : "a"; }
export const arrowBlock = (): string => { return 1; };
export function nested(): { a: { b: string } } { return { a: { b: 2 } }; }
declare const n: number;
export function ident(): string { return n; }
