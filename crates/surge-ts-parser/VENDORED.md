# Vendored oxc_parser

Verbatim copy of `oxc_parser` 0.128.0 from crates.io, wired in through
`[patch.crates-io]` in the workspace `Cargo.toml` so the package name, and with
it type compatibility with `oxc_ast`/`oxc_semantic`, stays unchanged.

Local changes are limited to making specific fatal parse errors recoverable
(record the error, return a dummy node, keep parsing) where tsc recovers and
reports a grammar diagnostic. Every patched site is marked with a
`// surge:` comment naming the tsc diagnostic it enables; `grep -rn "// surge:"`
lists the full delta against upstream.

When bumping oxc, re-copy the new upstream version and re-apply the marked sites.
