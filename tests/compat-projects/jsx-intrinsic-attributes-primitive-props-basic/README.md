# jsx-intrinsic-attributes-primitive-props-basic

A class component without `JSX.ElementAttributesProperty` takes its props from
the constructor's first parameter, joined with `IntrinsicAttributes` and
`IntrinsicClassAttributes<Instance>`. tsc's `isExcessPropertyCheckTarget` holds
for an intersection only when every constituent is an object type, so with
`string` props no attribute is excess: the relation fails on the first
constituent instead, and — the namespace declaring both intrinsic interfaces —
the report names it: TS2741 for the missing required `key` at the tag. With
object props the excess `x` is reported as usual.
