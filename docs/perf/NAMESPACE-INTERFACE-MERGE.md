# Namespace interface declaration merging (2026-08-20)

Correctness fix that is **implemented but gated off** (`SURGE_NS_IFACE_MERGE=1`),
with the measurement that says why, and the trace evidence that names the real
blocker — which turned out **not** to be the cycle truncation the counters first
suggested. Companion negative result on `SURGE_IFACE_CACHE_ALL` at the end.

## Problem

`collect_namespace_type_declarations_prefixed` registered a namespace's
interfaces first-wins, so a re-opened interface lost every block after the first.
`typescript.d.ts` splits ten interfaces in two — `Node`, `Type`, `Symbol`,
`Identifier`, `SourceFile`, `Signature`, `TypeReference`, `PrivateIdentifier`,
`SourceMapSource`, `SourceFileLike` — and the dropped half is in each case the
*service-method* block:

```ts
interface Type { flags: TypeFlags; symbol: Symbol; /* … */ }
interface Type { getFlags(): TypeFlags; getProperty(name: string): Symbol | undefined; /* … */ }
```

On tRPC this is worth **33 false positives** — the whole
`ts.Type.getProperty` / `ts.Symbol.getName` / `Node.getText` / `Identifier.text`
cluster in `packages/openapi` and `packages/upgrade`, which drive the TypeScript
compiler API. Merging the blocks takes tRPC from 825 diagnostics to **793 with
zero new false positives**.

## Why it is not the default

Interleaved A/B in one window, `.local-projects/trpc`:

| | wall |
| --- | ---: |
| merge off | 10.1s |
| merge on | 32.7s (**+223%**) |

## First reading of the counters (superseded — see the follow-up below)

`SURGE_ALLOCATION_CENSUS=1 SURGE_TIMINGS=1`, with and without
`SURGE_NS_IFACE_MERGE=1`:

| counter | off | on | |
| --- | ---: | ---: | --- |
| `interface_resolution_success_count` | 132,710 | 132,693 | unchanged |
| `lazy_reference_clean_expansion_count` | 7,997 | 7,999 | unchanged |
| `interface_resolution_degraded_count` | 89,819 | 2,430,238 | **27×** |
| `lazy_reference_degraded_expansion_count` | 19,528 | 1,266,359 | **65×** |
| `interface_member_declaration_visit_count` | 3,278,667 | 26,161,437 | 8× |
| `interface_own_property_map_alloc_count` | 223,331 | 2,563,733 | 11.5× |
| `generic_instantiation_count` | 1,511,493 | 3,779,201 | 2.5× |

The *clean* work does not move at all. Every added unit is a **degraded**
expansion, and a degraded expansion is deliberately never cached (see the
caching guardrails in `docs/PERFORMANCE_INVARIANTS.md`), so each peel re-expands
from scratch. *What* degrades is answered by the trace in the next section, not
by these counters — the obvious reading (the mutual `ts.Node`/`ts.SourceFile`
cycle the merge creates) is wrong.

Two facts worth keeping:

- `lib.dom.d.ts` has **zero** re-opened top-level interfaces, and this merge only
  applies inside `declare namespace`, so the DOM lib is untouched. The entire
  cost comes from `declare namespace ts` and `declare namespace React`.
- The merge must be computed from the parsed blocks **in one shot** and upserted,
  never folded into the table incrementally. The declaration table is rebuilt for
  every consuming module, and folding an already-merged result into itself
  re-appends its whole method set on each pass — that variant measured a further
  4× on top of the numbers above (40-84s).

## What the degradation actually is (2026-08-20 follow-up)

The first reading of the counters above blamed cycle truncation. It is **not**
that: `lazy_reference_blocked_count`, the peel-stack cycle guard, is
**unchanged** by the merge (1,701 → 1,703), and so is
`lazy_reference_clean_expansion_count`. `SURGE_TRACE_HAD_ERROR=1` with the merge
on names the real origin — 5.17M traced `had_error` creations, of which 1.97M are
`lookup-miss`, and those are dominated by **enum member types**:

| missing name | count |
| --- | ---: |
| `SyntaxKind.EndOfFileToken` | 1,181,892 |
| `SyntaxKind.SourceFile` | 590,946 |
| `SyntaxKind.ModuleBlock` | 53,878 |
| `SyntaxKind.Identifier` | 27,049 |
| `SyntaxKind.ModuleDeclaration` | 15,975 |

`typescript.d.ts` discriminates nearly every node interface with
`readonly kind: SyntaxKind.X`. The enum lowering emitted a union alias and an
ambient const but nothing for the *member* type, so each of those annotations
missed, tainted its interface as degraded, and made the expansion uncacheable —
which is why every peel redid it. The merge did not create the degradation; it
multiplied how often the permanently-degraded interfaces get pulled in.

The file-scope half of that gap is fixed (`fix(syntax): resolve enum member types
by lowering one alias per member`): `Enum.Member` aliases are now emitted, which
also restores discriminant narrowing for `interface W { kind: Label.Wide }`.

## Why the namespace half is still blocked

An enum inside a namespace registers qualified (`ts.SyntaxKind.SourceFile`), so a
bare `SyntaxKind.SourceFile` written *inside* that namespace needs the qualified
retry that `namespace_qualified_candidates` deliberately skips for already-dotted
names. Adding that retry is a five-line change and is byte-identical on its own.

Enabling **both** halves, however, does not finish: tRPC runs past 500s without
completing (each half alone is ~9-10s, output unchanged). The degradation was
acting as an accidental circuit breaker. Once `SyntaxKind.X` resolves, the
`ts` interface graph — 588 KB of mutually referencing declarations — expands
**structurally**, into real property maps, instead of being cut short.

So the blocker is not cycle tolerance but **eager structural expansion of
interfaces**. The fix is to keep an interface reference nominal (`Type::Reference`)
through a declaration graph of this size and peel only what a consumer actually
reads, which is what tsc does. Both the merge and the namespace enum retry become
affordable once that holds; neither needs changing itself.

## Validation of the gated-off default

With `SURGE_NS_IFACE_MERGE` unset the output is byte-identical to the previous
commit on all five corpora (ky 0, ofetch 1, zod 21, tRPC 825, unnamed 0), the
gate costs nothing measurable (tRPC wall median 9.91s → 10.04s, user 9.43s →
9.47s), and `cargo nextest run --workspace` (1764), the 105-target oracle sweep,
`oracle:test`, and `real:test` are green.

## Companion negative result: `SURGE_IFACE_CACHE_ALL` is still not a win

The extended interface-instantiation cache (check-phase only, every declaration
rather than the physical default lib) was re-measured in the same session because
a single run suggested −7.6% on tRPC. It is not real:

| | wall | user | RSS |
| --- | ---: | ---: | ---: |
| tRPC off → on | +1.7% | +0.4% | −0.7% |
| zod off → on | +0.9% | +1.2% | −0.2% |

(5 interleaved runs each, medians.) The mode is byte-identical on all five
corpora and passes the full sweep, and its hit count did improve from 2 to 1,312
— but most check-phase lookups still skip on a placeholder type argument
(`physical_interface_cache_skip_unresolved_argument_count` 2,888 → 49,578), so
there is nothing to win yet. Leave it opt-in. A single run is noise: load average
reached 13-18 during this session and identical binaries ranged 8s to 36s on
tRPC.
