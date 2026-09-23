const big = BigInt(1);
Promise.allSettled([Promise.resolve(1)]);
const matches: string[] = [..."ab".matchAll(/a/g)].map(m => m[0]);

// The directive adds ES2020 on top of the target's libs, nothing later.
"ab".replaceAll("a", "b");

export { big, matches };
