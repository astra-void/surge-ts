# unknown-guard-narrowing-basic

`narrowTypeByTypeof` / `narrowTypeByInstanceof` on an `unknown` subject: the
guard narrows it to the tag's type or the constructor's instance type, in an
`if`, after an early exit, and through an `asserts` call. surge dropped a
guarded `unknown` to its degradation sentinel *before* running the narrowers,
so the branch read as untyped and reported nothing. The sentinel is now only
the fallback for a guard no narrower could turn into a type.
