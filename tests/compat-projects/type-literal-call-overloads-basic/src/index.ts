// Every call signature of a type literal is an overload, as in an interface:
// a call resolves against the overload its arguments fit.
declare const overloaded: { (a: number): number; (a: string): string };
export const fromNumber: number = overloaded(1);
export const fromString: string = overloaded("x");
overloaded(true);
overloaded();

declare const byArity: { (a: string): void; (a: string, b: number): void };
byArity("a");
byArity("a", 1);
byArity("a", "b");
byArity();

interface Box {
    make: { (value: string): string[]; (value: number): number[] };
}
declare const box: Box;
export const made: number[] = box.make(1);
box.make(null);
