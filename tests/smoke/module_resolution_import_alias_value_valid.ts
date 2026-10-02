// @filename: src/a.ts
export function getName(): string { return "Ada"; }
// @filename: src/b.ts
import { getName as getUserName } from "./a"; let name: string = getUserName();
