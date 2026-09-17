# no-default-export-import-basic

tsc's `reportNonDefaultExport`: a default import from a source module with
no default export is TS1192, or TS2613 when the module exports a member of
the imported name. A declaration file's module can always be imported
through a synthetic default, and `{ default as x }` stays TS2305. surge
reported TS2305 for all of them, including the synthetic case.
