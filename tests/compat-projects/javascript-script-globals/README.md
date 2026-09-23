# javascript-script-globals

Under `allowJs` a JavaScript file listed in `files` is part of the program,
so the globals a script declares are visible to TypeScript files. Every
parameter of an untyped JavaScript signature is optional
(`isUntypedSignatureInJSFile`), so `greet()` and `greet("world")` are fine
and only the call with three arguments is TS2554 ("Expected 0-2 arguments").
