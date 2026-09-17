# parser-grammar-codes-basic

oxc raises these grammar errors with their TypeScript code and tsc's anchor,
but surge only reports a parser error under a code its catalog marks emitted;
every other one became a `surge::parser-error` the tsc profile hides. Each
file holds one such error (tsc reports them from the checker, so they appear
together).
