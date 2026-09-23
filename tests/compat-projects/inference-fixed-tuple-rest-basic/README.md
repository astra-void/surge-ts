# inference-fixed-tuple-rest-basic

A source signature whose rest parameter is a fixed tuple (`...args: []`, what
`Parameters<() => R>` resolves to) has exactly the tuple's positions: tsc's
`getParameterCount` counts them and infers nothing past them. surge read every
later position as `any`, so a zero-argument mock passed as a mutation function
bound `TVariables` to `any` instead of leaving its `void` default.
