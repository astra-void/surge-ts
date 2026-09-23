# element-access-argument-basic

An element access with nothing between the brackets is a parse error tsc
reports as TS1011 at the position just after the `[`, then keeps parsing.
Every instance in the file is reported; tsc checks nothing else in a program
with syntax errors, so the file has no other diagnostics.
