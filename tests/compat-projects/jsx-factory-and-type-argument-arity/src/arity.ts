declare function pick(value: number): number;
declare function pick<T, U>(value: T, other: U): T;

export const picked = pick<number>(1);
