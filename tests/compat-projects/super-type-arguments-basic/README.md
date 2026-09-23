# super-type-arguments-basic

tsc's parser reports type arguments on `super` itself as TS2754 on the
argument list, whether it is then called or accessed; type arguments on a
`super.method` call are fine.
