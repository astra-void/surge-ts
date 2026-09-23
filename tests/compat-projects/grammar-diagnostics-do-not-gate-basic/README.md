# grammar-diagnostics-do-not-gate-basic

Grammar errors tsc reports from its checker (TS1030 on a repeated modifier,
TS1016 on a required parameter after an optional one) are semantic
diagnostics: unlike a parse error they leave every other semantic diagnostic
in place (`GetDiagnosticsOfAnyProgram`).
