// @surge-args: --diagnosticProfile native
// @surge-expect: surge::unsupported-module-syntax TS2304
import type { User } from "./user";
let user: User = { name: "Ada" };
