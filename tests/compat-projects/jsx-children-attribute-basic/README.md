# jsx-children-attribute-basic

The element's body is the `children` attribute (named by
`ElementChildrenAttribute`). A lone child that does not fit is reported at the
child, unless every member of the children type is iterable — then it is
TS2745 at the tag. `string` is iterable, so a required `string` children type
takes TS2745 while an optional one (`string | undefined`) reports at the
child. Whitespace that spans a line break is not a child.

Children are excess only when the element writes an attribute: without one
the attributes object is not a fresh literal.
