# reference-lib-directive-basic

tsc's file loader (`filesparser.go`) adds every `/// <reference lib="…" />`
of every program file — a hand-written declaration file here, `@types/node`'s
`reference lib="es2020"` in practice — to the program's lib set, on top of the
libs the target or `lib` option selects. The name is matched
case-insensitively (`tsoptions.GetLibFileName`). Nothing beyond the named lib
is added: an ES2021 member still reports TS2550.
