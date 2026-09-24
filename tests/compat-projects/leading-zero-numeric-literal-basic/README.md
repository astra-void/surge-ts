# leading-zero-numeric-literal-basic

tsc's scanner rejects a `0` followed by digits in every file, strict or not:
all-octal digits are a legacy octal literal (TS1121, suggesting the `0o`
spelling), and any `8`/`9` makes it a decimal with a leading zero (TS1489).
When the previous token is `-` — unary or binary — the scanner folds it into
the suggestion (`-0o7`) and starts the span one character earlier. These are
scanner errors, so the program reports its syntactic diagnostics alone: the
TS2322 in `module.ts` is not reported.

Paired with `numeric-literal-forms-valid`.
