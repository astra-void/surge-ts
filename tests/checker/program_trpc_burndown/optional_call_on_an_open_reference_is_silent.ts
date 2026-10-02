type Setdown<C1 extends object = object> = (context: Partial<C1>) => void; declare function run<T extends object>(cb: Setdown<T>): void; run<any>(async (ctx) => { await ctx?.close?.(); });
