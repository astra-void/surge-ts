# jsx-children-arity-basic

tsc's `elaborateJsxComponents` splits the children prop's type into its
iterable and non-iterable members. A lone child against a type with a
non-iterable member is related to it and reported at the child — text that
does not fit is TS2747 there. A lone child against an iterable-only type is
TS2745 at the tag, and several children against a type with no iterable
member are TS2746 at the tag. Several children against an iterable member are
related one at a time, which `Either` passes.

A JSX text error is reported where the text node starts, leading whitespace
and line break included.
