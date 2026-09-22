# strict-mode-names-basic

tsgo checks every file as strict-mode code (the binder's `checkStrictMode*`
family): `eval`/`arguments` as a binding or write target is TS1100 in a script,
TS1215 in a module and TS1210 inside a class; a future reserved word used as an
identifier is TS1212/TS1214/TS1213 by the same split; `with` is TS1101 plus the
checker's TS2410; `delete name` is TS1102 even in a script; and `let` may not
name a `let`/`const` binding (TS2480).
