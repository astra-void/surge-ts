type Without<T, U> = T extends U ? never : T;
type A = Without<"a" | "b", "a">;
const bad: A = "a";
