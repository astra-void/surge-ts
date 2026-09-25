# erasable-syntax-only

Under `erasableSyntaxOnly`, the syntax that emits JavaScript is TS1294
(`shouldCheckErasableSyntax`): an enum that is not ambient (a `const enum`
included), an instantiated
namespace, an `import =` alias, a parameter property and an angle-bracket
type assertion. Ambient declarations and a namespace holding only types are
erased and stay silent.
