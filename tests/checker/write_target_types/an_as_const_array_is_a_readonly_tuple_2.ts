// …and is not assignable to a mutable array, which has its own code.
const values = [1, 2] as const;
export const mutable: number[] = values;
