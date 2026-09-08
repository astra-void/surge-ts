# instanceof-heritage-narrowing-basic

`x instanceof C` filtered a union by comparing each member's *name* to the
constructor's, which cannot see heritage: `TRPCClientError | Envelope` guarded
by `instanceof Error` matched nothing, so neither branch narrowed and the false
side of `props.result instanceof Error || …` still read the error arm. tRPC's
`loggerLink` writes exactly that `||`.

A member the name compare rejects is now re-tested against the constructor's own
instance type, which needs the checker context — so the guard joins the
predicate narrowing rather than the purely syntactic guards, and reaches a
property path (`props.result`) as well as a binding.

The re-test is asymmetric on purpose: the member must be assignable to the
instance *and* the instance not assignable back. `ErrorShaped` is shaped like
`Error` and relates in both directions, so it stays undecided and the negative
branch keeps it — pinned by `aLookAlikeIsNotAnInstance`.

`theComplementIsExactlyTheOtherArm` is the intentional error, and it is what
pins the fix: the false branch is `Envelope` alone, so reading `data` there
reports. Without the heritage re-test neither arm is dropped, the subject stays
a union, and surge says nothing — a union receiver that has the property on
*some* arm is a separate, still-open gap.
