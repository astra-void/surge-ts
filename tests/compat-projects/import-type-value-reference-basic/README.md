# import-type-value-reference-basic

Pins TS1361 (`'X' cannot be used as a value because it was imported using
'import type'`) and, just as importantly, where it must *not* fire: a type-only
import is exactly what makes `typeof greet`, `typeof Lib.greet`, and
`Lib.Options` legal in type positions. Only the two value references at the
bottom of `consumer.ts` are errors.

Not pinned here: `export const v = Lib;` — a value reference to a
`import type * as Lib` namespace binding. tsc reports TS1361 for it; surge
does not, because the namespace import installs type declarations for the
alias and the check keys off their absence. Tracked as a known gap rather than
weakened into a passing assertion.
