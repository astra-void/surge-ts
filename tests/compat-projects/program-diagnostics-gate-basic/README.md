# program-diagnostics-gate-basic

A `compilerOptions.types` entry that resolves to nothing is a location-less
file-inclusion error (TS2688), one of the program diagnostics
(`Program.GetProgramDiagnostics`). tsc computes no semantic diagnostics for a
program that has any, so the TS2322 in `index.ts` is not reported.
