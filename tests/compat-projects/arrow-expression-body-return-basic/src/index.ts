export const arrow = (): string => 1;
export const obj = (): { a: string } => ({ a: 1 });
export const cond = (c: boolean): string => c ? 1 : "a";
export const ok = (): number => 1;
export class C { m = (): string => 2; }
declare function take(cb: () => string): void;
take((): string => 3);
