// @surge-args: --diagnosticProfile native
// @surge-expect: surge::unsupported-module-syntax TS2304
import { getName } from "./user";
let value: string = getName();
