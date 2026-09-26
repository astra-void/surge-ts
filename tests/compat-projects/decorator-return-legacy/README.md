# decorator-return-legacy

tsc's `checkDecorator` under `experimentalDecorators`: the resolved call's return
type must be assignable to what `getLegacyDecoratorCallSignature` returns —
`typeof C | void` for a class decorator (TS1270), `void` for a property or
parameter decorator (TS1271). A decorator returning `any`, `void`, or a type
inferred from its argument (`keep<T>`) passes.
