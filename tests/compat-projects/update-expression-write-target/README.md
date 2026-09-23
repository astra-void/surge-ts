# update-expression-write-target

tsc's parser reads an assignment's target and a prefix `++`/`--` operand as a
left-hand-side expression only, so an update expression there ends the
statement: the assignment operator after `count++`, or the trailing operator
of `--count--`, is TS1005 `';' expected`, and a trailing operator that begins
the next statement with no operand after it is TS1109 on the following token.
They are parse errors, so the program reports its syntactic diagnostics alone.
oxc accepts the construct and reports an invalid write target, which surge
had numbered as the checker's TS2357/TS2364 and so let every semantic
diagnostic in the program through.
