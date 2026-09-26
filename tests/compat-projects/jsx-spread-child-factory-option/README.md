# jsx-spread-child-factory-option

A `{...spread}` child must be `any` or an array — not a tuple, not a union
(`checkJsxExpression`, TS2609). Under a JSX transform with the `jsxFactory`
option and no `jsxFragmentFactory`, every fragment is TS17016
(`checkJsxFragment`).
