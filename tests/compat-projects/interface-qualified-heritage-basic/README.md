# interface-qualified-heritage-basic

**Not a registered oracle preset.** This fixture reproduces a shape the checker
does not currently support; registering it in `scripts/oracle/compare-tsc.ts`
would fail `pnpm run oracle:sweep`.

It pins the qualified heritage clause (`interface X extends NS.Member`,
`class C extends NS.Base`) across local, `declare`, global-merged, and
namespace-imported namespaces, plus generic and nested-namespace bases.

Measured 2026-07-29 against the pinned oracle (tsc 7.0.2):

| Binary | Diagnostics | Result |
| --- | ---: | --- |
| tsc | 8 | reference |
| surge at `b38d381` | 17 | 13 false positives, 4 of tsc's 8 missed |

The 8 intentional errors in `src/errors.ts` pin inherited-member typing
(TS2322) and closedness (TS2339); everything in `src/index.ts` must stay
diagnostic-free.

Why it is not fixed: see the qualified-heritage entry under "Known limitations
discovered" in [REAL_PROJECT_COMPAT.md](../../../REAL_PROJECT_COMPAT.md). A
parse-side fix reaches exact 8/8 parity here (message text included) but
regresses zod by +77% wall time, because the newly-resolved bases more than
double the population of degraded interface resolutions, which are never
cached and so are recomputed at every use site. Re-measured 2026-07-29 on top
of the signature-context generic-instantiation cache
([docs/perf/SIGNATURE-CONTEXT-GENERIC-CACHE.md](../../../docs/perf/SIGNATURE-CONTEXT-GENERIC-CACHE.md)):
unchanged verdict — the added volume is *degraded* (uncacheable) work, so the
cache does not absorb it. The remaining lever is making those base expansions
resolve cleanly in generic contexts.

Third evaluation, 2026-07-29, on top of the distributive-conditional member
guards
([docs/perf/CLEAN-GENERIC-BASE-EXPANSION.md](../../../docs/perf/CLEAN-GENERIC-BASE-EXPANSION.md)):
still rejected. The guards cut heritage-context degraded resolutions
147,132 → 126,517, and the patch re-reached exact 8/8 parity here with zod
909 → 538 / tRPC 1872 → 1863 (no tsc TP lost), but zod `--jobs auto` remains
+59% wall / +26% peak (7/7 pairs). The dominant remaining degraded volume is
the analysis-phase import-less-scope family; its sound repair is blocked on
the dependency-declaration two-copy identity gap documented in the perf
report.

Neighboring behavior this fixture deliberately does **not** cover: a namespace
member reached through a namespace import with three or more segments
(`N.NS.Member`), which resolves correctly in type position since the
2026-07-29 namespace-import alias-table fix (gated by
`namespace-import-qualified-member-basic`).
