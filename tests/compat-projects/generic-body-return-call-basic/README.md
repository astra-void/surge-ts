# generic-body-return-call-basic

A generic function with no return annotation returns what its body returns,
instantiated for each call (`instantiateType(getReturnTypeOfSignature(
sig.target), sig.mapper)`). surge left the return of every such declaration at
its degradation sentinel, so a call to a generic helper like tRPC's
`getServerAndReactClient(appRouter)` typed nothing downstream.
