# tsc-parser-javascript-only-syntax

typescript-go's parser reports TypeScript-only syntax in a JavaScript file
(`checkJSSyntax`) as syntactic diagnostics, whether or not the file is
type-checked: type annotations, interfaces, enums, `import =`, `export =`,
non-null and `as`/`satisfies` expressions, TypeScript modifiers, signatures,
type parameters, `implements`, and decorators on both sides of `export`.
