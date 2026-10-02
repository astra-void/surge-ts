// @filename: src/a.ts
const version: string = "1"; export { version };
// @filename: src/b.ts
import { version } from "./a"; let current: string = version;
