# parser-classified-diagnostics-basic

oxc already classifies many TypeScript grammar failures: its `ts_error` helper
attaches a TS number and a span, and 76 diagnostics are raised that way. surge
discarded all of it — `parse_source_in` mapped each failure through
`error.to_string()`, so the code and the span were gone before the checker saw
them and every one surfaced as the un-numbered `surge::parser-error`.

The fix keeps `code` and the first label span on `ParserError` and renders the
message **from surge's own catalog rather than from oxc's string**. That matters:
of oxc's 64 single-literal messages, 53 are byte-identical to tsc's and 11 are
not — a missing trailing period, a backtick where tsc uses a quote. Rendering
from the catalog makes all 11 correct without touching oxc.

These are *grammar* errors, which tsc raises during checking, not parse
diagnostics. tsc reports them alongside semantic errors (its syntactic-error
gate in `program.go` skips semantics only for genuine parse failures), so no
suppression rule applies here.

## One construct per file

Several codes are masked when their triggers share a file: the first failure
consumes the construct that would raise the second. `indexer_annotation.ts` and
`indexer_parameters.ts` are split for exactly this reason, as are the four
single-construct files. Adding a second trigger to any of them silently drops a
code from this fixture's coverage.

## Known drift, recorded rather than hidden

Column is not exact on five of the twenty. surge anchors at oxc's label, tsc at
its own node: TS1092 33/34, TS1093 35/37, TS1096 27/28, and TS1173/TS1175 at the
class name (24) where tsc points at the offending clause (42). Line, code and
message agree, so the sweep gate passes; `--strictSpans` would not.

## Withheld

Two codes oxc classifies correctly are left `catalog-only` because this fixture
cannot witness them at parity, not because they are wrong:

- **TS1047** (`...rest?: string[]`) — tsc also emits TS2370 here, which surge
  does not, so the trigger cannot reach code-count parity.
- **TS1099** (`Array<>`) — surge reports the co-emitted TS2314 at line 2 column 1
  of a one-line file. That span bug predates this change; TS1099 itself matches.

A further 35 carry a code but no witness yet, and fall back to the un-numbered
parser diagnostic exactly as before.

**Verified false positive, deliberately not enabled:** `type T<> = string`
raises TS1098 in oxc and **nothing at all** in tsc. Enabling it would have added
a false positive; oxc also answers `[k: string]?: string` with TS1021 where tsc
says TS1005/TS1131.
