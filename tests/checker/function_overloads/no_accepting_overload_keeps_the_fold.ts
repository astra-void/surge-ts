// When no overload accepts the arguments the call keeps the fold's return.
declare function widen(v: string): string;
declare function widen(v: number): number;
export const a = widen(true);
