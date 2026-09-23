# import-attribute-resolution-mode

An `import type`/`export type` declaration with a `resolution-mode` import
attribute resolves its specifier in that mode (tsgo's
`getModeForUsageLocation`), so one file can reach both faces of a package with
`import`/`require` conditions. surge resolved every import in the file's own
mode.

- Lines 1 and 5 read `require.d.ts`; lines 2 and 6 read `import.d.ts`.
- Line 3 has no attribute and resolves in the ESM file's own mode:
  TS2305 for `RequireInterface`.
