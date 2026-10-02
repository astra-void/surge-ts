// @module: preserve
// @surge-args: --diagnosticProfile native
// @surge-expect: none
// @filename: pkg.d.ts
declare const auth: { sign(input: string): string };
export = auth;
// @filename: consumer.ts
import auth = require("./pkg");
const token: string = auth.sign("x");
