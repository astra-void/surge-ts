// @surge-args: --diagnosticProfile native
// @surge-expect: surge::parser-error TS2307
// @filename: a.ts
import { User from "./user";
// @filename: b.ts
import { User } from "./missing";
