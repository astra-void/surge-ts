// @filename: pkg.d.ts
declare const auth: { sign(input: string): string };
export = auth;
// @filename: index.ts
import auth = require("./pkg");
const token: string = auth.sign("x");
void token;
