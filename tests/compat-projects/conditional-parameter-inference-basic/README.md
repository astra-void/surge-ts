# conditional-parameter-inference-basic

A type parameter reached only through a conditional parameter type is
inferred from both of its branches, as tsc's `inferToConditionalType` does.
tRPC's `initTRPC.context<…>().create(opts?: ValidateShape<TOptions, …>)`
reaches `TOptions` only through the inner conditional's true branch; surge
left it uninferred, so every router's root types (`errorShape`,
`transformer`) degraded and the whole `AppRouter` went silent.
