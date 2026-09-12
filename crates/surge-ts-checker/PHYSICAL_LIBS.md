# Physical `lib.d.ts` Loading

**Superseded.** surge no longer loads the standard library from an installed
`typescript` package by default. It embeds its own version-pinned snapshot, and
an on-disk lib directory is used only when explicitly selected.

See [STANDARD_LIBS.md](STANDARD_LIBS.md) for the current architecture, source
precedence, `lib`/`target`/`noLib` handling, and the known checker-surface gaps.
