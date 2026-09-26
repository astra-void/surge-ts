# javascript-extends-type-arguments

A checked JavaScript class whose `extends` clause names a generic class with
the wrong number of type arguments. JavaScript cannot write type arguments on
the clause, so tsc asks for an `@extends` tag instead of reporting TS2314:
TS8026 when every type parameter is required (no tag, a tag without
arguments, a tag with too many), and TS8027 when some have defaults. The
reference is the clause itself even when a tag supplies the arguments.
