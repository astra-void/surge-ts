# jsx-intrinsic-attributes-constituent-basic

When the JSX namespace declares both `IntrinsicAttributes` and
`IntrinsicClassAttributes`, tsc's `reportErrorResults` drops the head message
for an attributes object related to an intersection holding them, so the
report names the constituent that misses the props (TS2741/TS2739), not the
whole `IntrinsicAttributes & Props`. `jsx-attributes-relation-basic` declares
only `IntrinsicAttributes` and keeps the TS2322 head.

A union of props is narrowed by the discriminant the attributes write before
the excess check (`findMatchingDiscriminantType`), so `radius` is excess on
the `square` member.
