// @useUnknownInCatchVariables: true
class E { issues: string[] = []; static assert(v: unknown): asserts v is E {} } export function f() { try { throw 1; } catch (err) { E.assert(err); const n: number = err.issues; } }
