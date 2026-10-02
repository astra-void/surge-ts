// @surge-args: --diagnosticProfile native
// @surge-expect: surge::unsupported-module-syntax TS2304
import { User as UserModel } from "./user";
let user: UserModel = { name: "Ada" };
