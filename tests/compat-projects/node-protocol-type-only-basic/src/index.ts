import type { Stats } from "node:fs";

declare function describe(stats: Stats): string;

const ok: Stats = { size: 1, isFile() { return true; } };
const label: string = describe(ok);
describe(42);

console.log(ok, label);
