type Without<T, U> = T extends U ? never : T;

type A = Without<"a" | "b" | "c", "a" | "c">;

const ok: A = "b";
const badA: A = "a";
const badC: A = "c";
