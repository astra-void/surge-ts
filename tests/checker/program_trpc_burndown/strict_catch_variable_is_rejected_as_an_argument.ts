// @useUnknownInCatchVariables: true
interface Opts { a: 1 } declare function f(msg: string, o?: Opts): void; export function g() { try { throw 1; } catch (err) { f('x', err); } }
