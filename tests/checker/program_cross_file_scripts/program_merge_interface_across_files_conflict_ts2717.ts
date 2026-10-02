// @filename: a.ts
// Global interfaces with the same name merge across files; a conflicting
// property type is reported once as TS2717 on the later declaration.
interface User { name: string; }
// @filename: b.ts
interface User { name: number; }
