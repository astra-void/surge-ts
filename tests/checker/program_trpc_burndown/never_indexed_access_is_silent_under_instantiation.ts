type Shape<T> = (T extends string ? { errorShape: 1 } : never)['errorShape']; type X = Shape<number>; export const y: X = null!;
