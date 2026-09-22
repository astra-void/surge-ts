interface Chain<T> {
    next<R1 = T, R2 = never>(ok?: (value: T) => R1, bad?: (error: unknown) => R2): Chain<R1 | R2>;
}
declare const chain: Chain<number>;
declare function start<R1 = string, R2 = never>(ok?: () => R1, bad?: () => R2): Chain<R1 | R2>;

export const a: boolean = chain.next(() => "x");
export const b: boolean = chain.next(() => "x", () => 1);
export const c: boolean = start(() => 1);
export const d: boolean = chain.next();
