# syntactic-diagnostics-gate-basic

tsc's `GetDiagnosticsOfAnyProgram` (program.go) reports a program's
syntactic diagnostics alone when there are any: option, global and semantic
diagnostics are never computed. The TS1477 parse error in `parse.ts` therefore
silences the type errors in `semantic.ts`.

An error oxc raises while parsing but tsc reports from its checker (a `const`
without an initializer, modifiers out of order) is a semantic diagnostic and
must not gate — see `grammar-diagnostics-do-not-gate-basic`.
