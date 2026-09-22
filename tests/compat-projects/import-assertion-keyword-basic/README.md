# import-assertion-keyword-basic

`assert { … }` in place of `with { … }` is a parse error in tsgo (TS2880, on
the keyword). As a syntax error it stops tsc checking the program, so it gets
a program to itself.
