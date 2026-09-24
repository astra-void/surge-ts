type T2 = [string, ...boolean];
type T4<T extends unknown[]> = [...T];
type T5 = [...string[]];
type T6 = [number, ...[number, string]];
type T7 = [...number, ...string];
type T8<T extends readonly unknown[]> = [boolean, ...T];
type T10 = [string, ...(number[] | string[])];
type T11 = [string, ...(number | string[])];
export {}
