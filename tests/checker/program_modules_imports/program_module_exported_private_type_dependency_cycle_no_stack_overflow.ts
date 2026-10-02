// @surge-args: --diagnosticProfile native
// @surge-expect: TS2322
// @filename: a.ts
interface A { next: B; }
interface B { next: A; }
export type Box = A;
// @filename: b.ts
import { Box } from "./a";
let box: Box = { next: { next: undefined } };
