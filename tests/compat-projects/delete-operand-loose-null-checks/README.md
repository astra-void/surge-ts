# delete-operand-loose-null-checks

tsc's `checkDeleteExpressionMustBeOptional` (TS2790) runs only under
`strictNullChecks`: without it every property may be deleted. A `readonly`
property is still TS2704 whatever the option.
