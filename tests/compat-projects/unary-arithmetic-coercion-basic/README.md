# unary-arithmetic-coercion-basic

`TS2356` ("An arithmetic operand must be of type 'any', 'number', 'bigint' or an
enum type") is the `++`/`--` operand rule. Unary `+`/`-` *coerce*: tsc accepts
any operand and types the result `number`. surge applied the rule to `+`/`-`
instead, so every coercion of a non-numeric value reported, and returned
`unknown` for the result on top of it.

`selector` is the shape this came from — tanstack-query writes
`(data) => [data, +data]` against a `(data: string) => [string, number]`
contextual type, where `data` is a `string` by contextual typing and `+data` is
the tuple's `number`.

`coercesToNumber` pins the result type: the branch used to fall through to
`unknown`, which silently disabled every downstream check on the value.

The `++`/`--` rule itself is the inverse gap and is *not* closed here: surge
reports nothing for `text++`, where tsc reports `TS2356`. Adding it is a new
check with its own false-positive surface, so it is recorded rather than
smuggled in. Do not read this project's green result as covering it.
