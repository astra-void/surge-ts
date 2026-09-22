# export-assignment-conflicts-basic

tsc's `checkExternalModuleExports`: `export =` in a module that also exports a
value is TS2309 on the assignment. A type-only export does not conflict.
CommonJS files, so `export =` itself is legal.
