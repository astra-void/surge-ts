declare function pick<O, K extends keyof O>(o: O, k: K): O[K];
export const value: string = pick({ p: 1, q: "s" }, "q");
