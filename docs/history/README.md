# Project history

**Everything in this directory is a historical record. None of it describes
current behavior.** These are milestone notes and superseded support lists that
were moved out of the top-level documents so they could not be mistaken for the
current state.

For what is true now, read [CURRENT_STATUS.md](../../CURRENT_STATUS.md).

| Document | What it holds |
| --- | --- |
| [ARCHITECTURE-VERSION-NOTES.md](ARCHITECTURE-VERSION-NOTES.md) | The `v0.4x`–`v1.2.5` milestone notes formerly in `ARCHITECTURE.md`: crate splits, module decompositions, declaration-ingestion milestones. |
| [REAL_PROJECT_COMPAT-HISTORY.md](REAL_PROJECT_COMPAT-HISTORY.md) | The `v0.60`–`v0.85` milestone log, the per-feature `v0.7x`/`v0.8x` notes, and the superseded "current baseline" support lists formerly in `REAL_PROJECT_COMPAT.md`. |

Two other historical records stayed where they are, because they are read as
part of their own subject rather than as project history:

- [STRICT_DRIFT_INVENTORY.md](../../STRICT_DRIFT_INVENTORY.md) — §§ 1–11 are a
  dated log of earlier strict sweeps, under an explicit `# Historical log`
  heading; the current snapshot is at the top of the same file.
- [docs/perf/](../perf/) — point-in-time optimization investigations, each
  carrying its own historical banner. See
  [docs/perf/README.md](../perf/README.md).

## Reading historical notes safely

- **`v0.x` / `v1.x` labels are internal milestone markers**, not releases,
  tags, or crate versions. See
  [CURRENT_STATUS.md § Versioning](../../CURRENT_STATUS.md#versioning).
- **A "not supported" line in a historical note is not evidence of a current
  gap.** Several such lines have since landed. Check
  [CURRENT_STATUS.md § Known limitations](../../CURRENT_STATUS.md#known-limitations)
  or probe it against the oracle before repeating it.
- **Historical wall-clock and memory figures are not comparable to current
  ones** — different commits, sometimes different fixture checkouts, sometimes
  a different default lib path.
