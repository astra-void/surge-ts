# jsx-children-arity-unnamed-basic

Without `JSX.ElementChildrenAttribute` the element's body forms no attribute,
so a required `children` prop is missing (TS2741). `elaborateJsxComponents`
still checks the body under the name `children` once the relation has failed,
reading the attributes' missing `children` as `unknown`: several children
against a single-child type are TS2746 and one child against an array type is
TS2745, each instead of the TS2741. When the relation holds nothing is
elaborated, and a failure elsewhere (`id`) is reported beside the arity error.
