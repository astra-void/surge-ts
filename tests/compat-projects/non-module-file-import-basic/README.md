# non-module-file-import-basic

Importing a program file that is a script (no import or export) reports TS2306 at the module specifier for every import form that binds a name; a side-effect import does not (tsc's `resolveExternalModule`).
