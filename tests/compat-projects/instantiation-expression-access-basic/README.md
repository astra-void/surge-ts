# instantiation-expression-access-basic

`f<T>.name` is a syntax error in tsc's parser (TS1477), anchored at the type
argument list. As a parse error it stops tsc checking the program, so it gets
a program to itself.
