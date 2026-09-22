# class-type-parameter-scope-basic

A generic class's members are checked like any other class's (they were
skipped entirely), with the class type parameters in scope — and out of scope
in a static member, which tsc reports as TS2302 whether the parameter is named
by an annotation, a signature, a body or an accessor. A static member's own
type parameter of the same name still shadows the class's.

The fixture also pins what the expression forms those bodies exposed report: a
computed key no member name can model (`[f()]`, a template with a
substitution) still checks its key and its value, a `for` update expression and
a comma expression check every operand, and a property whose annotation did not
resolve is exempt from TS2564 (tsc's `checkPropertyInitialization` skips an
error type).
