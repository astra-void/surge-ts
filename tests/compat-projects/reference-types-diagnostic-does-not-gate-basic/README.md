# reference-types-diagnostic-does-not-gate-basic

An unresolved `/// <reference types>` is an inclusion error located in the
file that wrote it. tsc reports it with that file's semantic diagnostics
(`GetIncludeProcessorDiagnostics`), not with the program diagnostics, so the
TS2322 beside it is still reported.
