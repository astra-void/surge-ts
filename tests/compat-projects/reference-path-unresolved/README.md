# reference-path-unresolved

tsgo's `getSourceFileFromReference` for a `/// <reference path>`, reported at
the path with the referencing file's semantic diagnostics. A path with an
extension must be a supported one (a JavaScript file without `allowJs` is
TS6504, anything else TS6054), must exist (TS6053, quoting the path as
written — a rooted one too) and must not be the file itself (TS1006). An
extensionless path is completed by `.ts`, `.tsx` or `.d.ts` alone, or is
TS6231. A resolved reference loads its file even outside `include`, and a
declaration file's own missing reference is reported there.
