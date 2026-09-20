# binding-element-default-basic

tsc's `checkBindingElement`: the initializer of `{ a = value }` in a
parameter is contextually typed by, and has to be assignable to, the type the
pattern reads at that position. surge kept only a flag saying a default was
written, so nothing about it was checked. A whole-value mismatch is reported
on the binding element; one inside the initializer where it occurs. Nothing
is checked beneath a nested pattern over a possibly-missing property, which
is itself the error (TS2339). An immediately invoked function reads the
widened literal type of its arguments.
