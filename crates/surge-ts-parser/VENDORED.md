# Vendored oxc_parser

Verbatim copy of `oxc_parser` 0.128.0 from crates.io, wired in through
`[patch.crates-io]` in the workspace `Cargo.toml` so the package name, and with
it type compatibility with `oxc_ast`/`oxc_semantic`, stays unchanged.

Local changes are limited to making specific fatal parse errors recoverable
(record the error, return a dummy node, keep parsing) where tsc recovers and
reports a grammar diagnostic, and to one added entry point:
`Parser::parse_jsdoc_type`, which reads a JSDoc type expression in place
inside a comment (tsgo's `parseJSDocType`: the `*` type, `...T`, `T=`,
`Name.<T>` and the leading `*` of each continued line) so its spans are the
file's own. Every patched site is marked with a `// surge:` comment naming
what it enables; `grep -rn "// surge:"` lists the full delta against upstream.

When bumping oxc, re-copy the new upstream version and re-apply the marked sites.
