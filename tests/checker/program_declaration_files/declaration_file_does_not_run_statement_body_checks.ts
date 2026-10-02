// @filename: src/index.ts
// Only the grammar error for a non-`declare` top-level statement; no
// TS1155 for the missing initializer.
let x: number = 1;
// @filename: types/globals.d.ts
const missingInit: number;
