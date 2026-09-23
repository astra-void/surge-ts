# inference-rest-tuple-contextual-parameters

A type parameter that types a rest parameter (`...args: U`) is inferred from
the trailing arguments as one tuple (`getSpreadArgumentType`), and a callback
typed `(...args: U) => T` then reads it position by position:

- `assignContextualParameterTypes` gives an unannotated callback parameter the
  rest tuple's element at its position (`getTypeAtPosition`), and a rest
  parameter the tuple itself; a rest typed by a bare type parameter is
  instantiated without fixing it.
- Inferring back from the callback (`applyToParameterTypes`) builds its
  parameters from that position on into one tuple (`getRestTypeAtPosition`),
  where an optional parameter is an optional element, so
  `run((foo: string, bar?: number) => "x", "foo", undefined)` keeps
  `U = [string, undefined]`.

This is the shape of lib `Promise.try`. The two intentional errors — `42`
against the callback's `string` parameter, and `foo * 2` on the inferred
`string` — are tsc errors too.
