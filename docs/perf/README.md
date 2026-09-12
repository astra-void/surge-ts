# Performance investigation reports

**Every file in this directory is a point-in-time record.** Each documents one
investigation as it stood on its own date: the measurements taken, the changes
landed, and — often more valuable — the designs that were measured and
**rejected**. None of them describes the current state, and their counts
(diagnostic totals, preset totals, wall-clock medians) are stale by
construction.

- Current state: [CURRENT_STATUS.md](../../CURRENT_STATUS.md)
- Benchmark methodology and the recorded runs: [BENCHMARKS.md](../../BENCHMARKS.md)
- The rules these investigations must obey: [PERFORMANCE_INVARIANTS.md](../PERFORMANCE_INVARIANTS.md)
- Retained-memory model: [MEMORY_REGIONS.md](../../crates/surge-ts-checker/MEMORY_REGIONS.md)

These reports are kept intact deliberately. A rejected design with its
measurement attached is the cheapest way to stop the same idea being
re-proposed, and several entries here exist purely to record *why* an obvious
optimization does not work.

## Design references (not status reports)

| Document | Subject |
| --- | --- |
| [SPECULATIVE-TRANSACTIONAL-CHECKING.md](SPECULATIVE-TRANSACTIONAL-CHECKING.md) | Design reference for parallelizing semantic analysis |
| [MEMBER-LAZY-EXPANSION.md](MEMBER-LAZY-EXPANSION.md) | Member-level lazy interface expansion (`SURGE_LAZY_IFACE_MEMBERS`, Stage 1 opt-in) |

## Investigation reports, newest first

| Document | Date | Subject |
| --- | --- | --- |
| [TANSTACK-QUERY-PROGRAM-MEMO-2026-09-12.md](TANSTACK-QUERY-PROGRAM-MEMO-2026-09-12.md) | 2026-09-12 | Program-lifetime interface memo on by default: the three soundness conditions it needed, tanstack-query 40G → 6G instructions and 1.03 GB → 189 MB; where the time goes now |
| [LOADER-PARSE-HANDOFF.md](LOADER-PARSE-HANDOFF.md) | 2026-09-11 | Handing the module-graph scan's parses to the checker instead of re-parsing; the per-file text sweeps around it. ofetch profile by phase |
| [NAMESPACE-INTERFACE-MERGE.md](NAMESPACE-INTERFACE-MERGE.md) | 2026-08-20 | Reopened-namespace interface merging. **Note:** the merge described there as gated off is now **on by default** (opt-out `SURGE_NS_IFACE_MERGE=0`). |
| [CLEAN-GENERIC-BASE-EXPANSION.md](CLEAN-GENERIC-BASE-EXPANSION.md) | 2026-07-29 | Distributive-conditional member guards; degradation provenance |
| [SIGNATURE-CONTEXT-GENERIC-CACHE.md](SIGNATURE-CONTEXT-GENERIC-CACHE.md) | 2026-07-29 | Signature-context generic-instantiation cache |
| [TRPC-CANONICALIZE-LEAF-PROBE.md](TRPC-CANONICALIZE-LEAF-PROBE.md) | 2026-07-28 | Path-canonicalization leaf probes |
| [TRPC-FRONTEND-LOADER.md](TRPC-FRONTEND-LOADER.md) | 2026-07-20 | Loader-side parallel reads and probe collapse |
| [TRPC-5S-FINAL.md](TRPC-5S-FINAL.md) | 2026-07-18 | tRPC 5-second mission, session report |
| [TRPC-5S-REPORT.md](TRPC-5S-REPORT.md) | — | tRPC 5-second program, engineering report |
| [TRPC-5S-BASELINE.md](TRPC-5S-BASELINE.md) | — | tRPC 5-second program, Stage 0 baseline |
| [TRPC-THIN-PRELIMINARY-VALUES.md](TRPC-THIN-PRELIMINARY-VALUES.md) | — | Thin exportable-value collection |
| [TRPC-LAZY-DTS-VALUES.md](TRPC-LAZY-DTS-VALUES.md) | — | Lazy library value annotations |
| [TRPC-DEFERRED-RESOLUTION.md](TRPC-DEFERRED-RESOLUTION.md) | — | Deferred resolution, engineering report and blocker |
| [TRPC-FINE-CONFLICT-DIGEST.md](TRPC-FINE-CONFLICT-DIGEST.md) | — | Fine conflict digest; stale-replay blocker |
| [TRPC-ORDERED-DELTA-REPLAY.md](TRPC-ORDERED-DELTA-REPLAY.md) | — | Ordered-delta pipelined replay |
| [TRPC-STC-REPORT.md](TRPC-STC-REPORT.md) | — | Speculative transactional checking, engineering report |
| [TRPC-ALLOCATION-VOLUME-REPORT.md](TRPC-ALLOCATION-VOLUME-REPORT.md) | — | Allocation-volume program |

Dates marked `—` are recorded inside the document rather than in its title.
