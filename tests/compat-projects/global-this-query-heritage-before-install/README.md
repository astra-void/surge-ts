# global-this-query-heritage-before-install

`runtime.d.ts` is a module that reopens the global `Stream` with a base read off
the global object (`typeof globalThis extends { Stream: infer S } ? S : never`),
the shape bun-types uses for `ReadableStream`/`WritableStream`. The base is the
`Stream` constructor, so every `Stream<W>` inherits `prototype` and the
construct signature, and `W` still relates both ways because `write` is a
method.

The heritage clause resolves under the module's scope, where `_Stream` lives.
surge builds the global object only once modules are bound, after the global
interfaces are first expanded, so a `typeof globalThis` read before then is
degraded rather than clean (a clean answer would be interned for the whole run),
and a context recovered in the check phase reads the program's final global
table.

The one intentional error binds `prototype` to `number`.
