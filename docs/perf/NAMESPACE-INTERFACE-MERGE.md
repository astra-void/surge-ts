# Namespace interface declaration merging (2026-08-20)

Correctness fix that is **implemented but gated off** (`SURGE_NS_IFACE_MERGE=1`),
with the measurement that says why, and the counter evidence that names the real
blocker. Companion negative result on `SURGE_IFACE_CACHE_ALL` at the end.

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

## Root cause (counter diff, one binary, gate on/off)

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
from scratch.

What degrades is the **mutual cycle the merge creates**: merged `ts.Node` gains
`getSourceFile(): SourceFile`, and merged `ts.SourceFile` returns `Node`. Before
the merge `ts.Node` was a three-property nominal type whose only cycle was
`parent: Node`, which the lazy reference absorbs.

Two facts worth keeping:

- `lib.dom.d.ts` has **zero** re-opened top-level interfaces, and this merge only
  applies inside `declare namespace`, so the DOM lib is untouched. The entire
  cost comes from `declare namespace ts` and `declare namespace React`.
- The merge must be computed from the parsed blocks **in one shot** and upserted,
  never folded into the table incrementally. The declaration table is rebuilt for
  every consuming module, and folding an already-merged result into itself
  re-appends its whole method set on each pass — that variant measured a further
  4× on top of the numbers above (40-84s).

## The real fix

Cycle-tolerant interface resolution: a reference to a sibling interface must stay
nominal while the enclosing body resolves, instead of truncating the cycle and
tainting the result `had_error`. That makes the merged expansions *clean*, and
therefore cacheable, which is the only reason the current numbers are what they
are. Nothing in the merge itself needs to change. cf. the dormant
`feat/cycle-tolerant-type-resolution` branch.

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
