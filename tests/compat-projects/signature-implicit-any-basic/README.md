# signature-implicit-any-basic

A type-level signature (construct, call, method, function or constructor
type) with no return type, an unannotated parameter, or a parameter default
still declares a signature: the missing pieces are implicit `any`, reported
as TS7013/TS7020/TS7010/TS7006 under `noImplicitAny`, and a default is
TS2371. surge's parser dropped any such signature, so every `new`/call
through it was a false TS2351/TS2349/TS2339 and a type alias written that way
vanished (TS2304).
