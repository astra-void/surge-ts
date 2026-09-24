# invalid-write-target-basic

Write targets and rest positions tsc rejects as grammar errors — an
assignment to a non-reference (TS2364) or an optional chain (TS2779), an
update of one (TS2357/TS2777), a rest parameter or element that is not last
(TS1014/TS2462). oxc gives these no TypeScript number, so surge classifies
its message and re-anchors it where tsc does (parentheses kept, a rest element
at its name). One case per file; `invalid-write-target-recovery-basic` puts
many in one file.
