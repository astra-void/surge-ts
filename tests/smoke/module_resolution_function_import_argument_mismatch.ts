// @filename: src/a.ts
export function greet(name: string): void { }
// @filename: src/b.ts
import { greet } from "./a"; greet(1);
