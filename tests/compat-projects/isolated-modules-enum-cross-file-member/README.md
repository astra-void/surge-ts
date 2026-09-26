# isolated-modules-enum-cross-file-member

tsc's name resolver looks a bare name in an enum member initializer up in the
merged enum's exports (`Resolve`, `case ast.KindEnumDeclaration`), so a member
another file's declaration of a global enum declares resolves there too. Under
`isolatedModules` (or `verbatimModuleSyntax`) such a reference is TS1281, since
a single-file transpiler cannot see it; a qualified `Color.Green`, a member of
the same file, and an initializer in an ambient declaration are not reported.
surge reported each such name as TS2304.
