export function arithmetic<T, N extends number, S extends string, B extends bigint>(t: T, n: N, s: S, b: B) {
    let a!: any;
    const r1 = a * t;
    const r2 = t - 1;
    const r3 = n * 2;
    const r4 = s * 2;
    const r5 = b * b;
    const r6 = -t;
    let x = 1;
    x *= t;
    const r7 = t << a;
    const r8 = t + 1;
    const r9 = n + 1;
    const r10 = s + 1;
    return [r1, r2, r3, r4, r5, r6, x, r7, r8, r9, r10];
}
