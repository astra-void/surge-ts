# parameter-property-default-required-basic

A constructor parameter property written with a default
(`readonly encoder: Encoder = noopEncoder`) declares a *required* instance
member. The parameter is optional at the call site, the property never is. The
parser folds a default into the parameter's `optional` flag so arity accepts the
omitted argument, and the class's instance side read that same flag, so every
such member came out optional and `param.encoder.encode(...)` reported TS18048.

`omittingTheDefaultedArgumentIsStillFine` is the control for the half that has
to keep working: the parameter is still optional at the call.

`theOptionalOneStillNeedsAGuard` is the intentional error and pins the
direction: a parameter property written with `?` really is optional, so reading
through it without a guard still reports.

Found while burning down the drizzle-orm corpus — closing the entity-guard
narrowing in `generic-class-entity-guard-basic` exposed this one, because
`is(p, Param)` had until then narrowed to `any` and hidden it.
