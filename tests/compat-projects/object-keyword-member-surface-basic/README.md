# object-keyword-member-surface-basic

The `object` keyword was lowered to surge's degradation sentinel, the same target
used for `intrinsic` and unparseable annotations. A sentinel suppresses
assignability and property lookup by design, so every read off an
`object`-typed value and every assignment out of one was silently accepted,
including through a `T extends object` constraint — which is how it reached
tRPC's `TRPCBuilder<TContext extends object, TMeta extends object>`.

`object` is every non-primitive, and the empty object type already models that
member surface exactly: a property read off it is an error and an assignment out
of it is checked. The last two declarations pin that target, since `{}` was
already exact while `object` reported nothing.

Not pinned here, and a deliberate difference rather than an oversight: assigning
a primitive *into* `object` (`const p: object = "str"`) is an error tsc reports
and surge does not. `{}` accepts primitives, so routing `object` through it
keeps that gap. Closing it needs a representation that rejects primitives on the
inbound side, which is a separate change.

Message drift is expected on the three `object` lines: surge renders the type as
`{}` where tsc writes `object`. The file, code and line match. A display name
would have to be carried on the lowered type, and display identity participates
in canonical-store sharing here, so it is not a cosmetic-only change.
