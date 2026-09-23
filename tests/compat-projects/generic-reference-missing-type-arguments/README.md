# generic-reference-missing-type-arguments

tsc's `getTypeFromClassOrInterfaceReference` reports a wrong type-argument
count on the type reference, naming the generic type with its parameters
(`Generic type 'C<T>' requires 1 type argument(s)`). surge reported a generic
class or interface at its declaration — in the referencing file, so at a line
that is not the reference — and under the local name of an import (`'a'` for
`import a = require(...)`). Too many arguments are reported the same way.
