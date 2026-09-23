// A signature present in every constituent keeps its parameters and unions
// the return types.
declare const sameParameters: { (a: number): number } | { (a: number): Date };
export const sameResult: number | Date = sameParameters(1);
sameParameters("x");

// A constituent taking a prefix of the parameters matches partially: the
// longer signature is the union's, so the arity is its arity.
declare const fewerElsewhere: { (a: string): string } | { (a: string, b: number): number };
fewerElsewhere("a");
fewerElsewhere("a", 1);

declare const optionalVsFewer: { (a: string, b?: number): string } | { (a: string): number };
optionalVsFewer("a");
optionalVsFewer("a", 1);
optionalVsFewer();

// A rest parameter matched by a fixed signature leaves the fixed one.
declare const restVsFixed: { (...a: string[]): string } | { (a: string, b: string): number };
restVsFixed("a");
restVsFixed("a", "b");
restVsFixed("a", "b", "c");

declare const restVsShorter: { (a: string, ...b: number[]): string } | { (a: string): number };
restVsShorter("a", 1, 2);
restVsShorter();
restVsShorter("a", "b");

type F3 = (a: string, ...rest: string[]) => void;
type F4 = (a: string, b?: string, ...rest: string[]) => void;
declare const restAndOptional: F3 | F4;
restAndOptional("a", "b", "c");

// Nothing common to every constituent: the signatures combine position by
// position into the intersection of what each takes there.
declare const restObjects: ((...objs: { x: number }[]) => number) | ((...objs: { y: number }[]) => number);
restObjects({ x: 0, y: 0 }, { x: 1, y: 1 });
restObjects({ x: 0 });

declare const leadingThenRest:
    | ((a: { x: number }, ...objs: { y: number }[]) => number)
    | ((...objs: { x: number }[]) => number);
leadingThenRest({ x: 0 }, { x: 0, y: 0 });
leadingThenRest();

declare const optionalPair: ((a?: { x: number }, b?: { x: number }) => number) | ((a?: { y: number }) => number);
optionalPair({ x: 0, y: 0 }, { x: 0 });

declare const extraRest: ((a?: { x: number }, b?: { x: number }) => number) | ((...a: { y: number }[]) => number);
extraRest({ x: 0, y: 0 }, { x: 0, y: 0 }, { y: 0 });

declare const disjoint: { (a: number): number } | { (a: string): Date };
disjoint(1);

// Overloads in every constituent match overload by overload.
declare const bothOverloaded: { (a: number): number; (a: string): string } | { (a: number): Date; (a: string): boolean };
export const overloadResult: string | boolean = bothOverloaded("hello");
bothOverloaded(true);
bothOverloaded();

declare const oneOverloaded: { (a: number): number } | { (a: number): Date; (a: string): boolean };
oneOverloaded("hello");

declare const twoOverloaded: { (a: number): number; (a: string): string } | { (a: boolean): Date; (a: object): Date };
twoOverloaded(1);

// A method read off a union receiver is called through the same signatures.
export function methods(z: { f(): void } | { f(x?: string): void; g(): void }) {
    z.f();
    z.f("hello");
}

export function requiredVsOptional(z: { f(x: string | undefined): void } | { f(x?: string): void }) {
    z.f("hello");
    z.f(undefined);
    z.f(1);
}

export function disjointMethods(z: { f(x: number): void } | { f(x: string): void }, value: string | number) {
    z.f(value);
}
