// The elements keep their literal types through `ReadonlyArray<T>`: the const
// context is what the assertion was written for, and a tuple's element type is
// the union of its elements rather than two candidates that collapse.
declare function firstOf<T>(values: ReadonlyArray<T>): T;
const modes = ["fast", "slow"] as const;
export const mode: "fast" | "slow" = firstOf(modes);
