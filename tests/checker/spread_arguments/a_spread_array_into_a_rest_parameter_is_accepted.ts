declare const parts: string[];
declare function joinAll(...pieces: string[]): string;
export function f() { return joinAll(...parts); }
