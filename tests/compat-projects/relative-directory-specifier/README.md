# relative-directory-specifier

A relative specifier ending in `/`, or whose last component is `.` or `..`,
names a directory (tsgo's `normalizePathForCJSResolution`), so resolution
skips the file lookups and takes the directory's index. surge probed `a.ts`
beside the directory first.

- `.`/`./` from `src/a/test.ts` and `..`/`../` from `src/a/b/test.ts` all
  resolve to `src/a/index.ts`, so `file` (only in `src/a.ts`) is TS2305.
- `src/lone` has no index: `.` from `src/lone/test.ts` is TS2307 even though
  `src/lone.ts` exists.
