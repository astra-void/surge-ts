# callable-interface-overload-member-call-basic

A call through a member typed by an interface with several generic call
signatures resolves over those signatures in order (tsc's `chooseOverload`,
`hasCorrectArity` first). surge instantiated the member's first written
signature instead, so `holder.list.queryOptions()` answered with the overload
that requires its options argument.
