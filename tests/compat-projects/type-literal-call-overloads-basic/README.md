# type-literal-call-overloads-basic

A type literal's call signatures are overloads exactly as an interface's are
(`resolveAnonymousTypeMembers` keeps every `(…): T` member), so a call on a
value typed `{ (a: number): number; (a: string): string }` resolves against
the overload its arguments fit (`chooseOverload`): its result is that
overload's return type, an argument no overload accepts is TS2769, and a call
no overload's arity admits is reported with the smallest and largest arity
among the candidates (`getArgumentArityError`).

Every error below is also a `tsc` error.
