// @surge-args: --diagnosticProfile native
// @surge-expect: surge::unsupported-module-syntax TS2304
import { User } from "./user";
let user: User = { name: 123 };
