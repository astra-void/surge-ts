# declaration-flow-narrowing-basic

Three flow facts tsc applies at a declaration or an assignment that surge did
not:

- An annotated union declaration is narrowed by *any* initializer, not only a
  literal one: `let client: Persisted | undefined = persisted` starts out as
  `Persisted`, also inside an arrow passed as an argument.
- A write to a property checks the property's *declared* type: after
  `if (o.flag === undefined)` the read type is `undefined`, but `o.flag = true`
  assigns to `boolean | undefined`.
- A `const` bound to a condition stands for that condition also as one operand
  of `&&`/`||`/`!` in an `if` (`isRefetch && mode === 'reset'`), not only as the
  whole condition.

The two intentional errors pin that the unguarded access and the wrongly typed
assignment still report.
