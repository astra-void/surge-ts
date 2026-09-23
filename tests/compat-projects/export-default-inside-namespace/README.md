# export-default-inside-namespace

tsc's `checkExportAssignment` reports an `export default` directly in a
namespace body as TS1319 and an `export =` there as TS1063, and returns before
anything else — the exported expression is never resolved, and the
ECMAScript-module check (TS1203) is never reached. surge resolved the
expression (TS2304 for each name) and reported TS1203 for `export =`.
