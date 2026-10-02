interface E { data: string } declare function isE(v: unknown): v is E; export function f(err: Error) { if (isE(err)) { const s: string = err.data; err.message; } }
