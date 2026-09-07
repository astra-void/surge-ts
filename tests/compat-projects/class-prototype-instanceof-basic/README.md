# class-prototype-instanceof-basic

A class value carries `prototype`, typed as the instance — the shape
`Object.setPrototypeOf(this, C.prototype)` reads. surge's static side listed only
the declared static members, so every such call was a `TS2339`.

`x instanceof C` matches a union member by *name*, which cannot see that
`ClosedError extends Error`. When no member names the constructor, the guard
still proves the value is an object, so the members that definitely are not one
— primitives, `null`/`undefined` — are dropped and the rest kept. Without that
`Maybe<ClosedError>` stayed nullable through the guard and `error.message` read
as `string | undefined`.

`x instanceof Array` is the same name test seen from the other side, and it
decides array-*ness*, not a nominal name: an array member renders as `T[]` and a
tuple as `[A, B]`, so comparing the name rejected both and
`messageOrMessages instanceof Array ? … : [ … ]` narrowed nothing — `messages`
kept the whole union and `.length` was a false `TS2339`.

The single intentional error is `prototypeIsNotTheStaticSide`: `prototype`
really is the instance type, so binding it to a `number` reports.

(The `Marker` pair keeps that check on a class with a modelled instance shape;
`ClosedError` inherits from the lib `Error`, whose instance surge treats
permissively.)
