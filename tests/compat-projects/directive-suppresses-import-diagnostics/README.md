# directive-suppresses-import-diagnostics

`// @ts-ignore` and `// @ts-expect-error` suppress the diagnostics an import
declaration reports while its module is bound — TS2307 for an unresolved
module, TS2305 for a missing export — exactly as they suppress the rest of the
file's semantic diagnostics. surge binds imports before the per-file check
that applied the directives, so those diagnostics escaped them.

- Lines 2, 4 and 7 are suppressed; the undirected TS2307 on line 5 and TS2305
  on line 8 are reported.
