// @filename: external.ts
// `import * as z from "./external"; export { z }` (zod's index.d.ts shape)
// re-exports the namespace's type members, so a type-only named import of `z`
// can reference them as qualified names instead of reporting TS2305.
export interface Payload { value: string }
// @filename: barrel.ts
import * as z from "./external";
export { z };
// @filename: main.ts
import type { z } from "./barrel";
const p: z.Payload = { value: "ok" };
export default p;
