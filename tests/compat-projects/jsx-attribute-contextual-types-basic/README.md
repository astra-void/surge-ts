# jsx-attribute-contextual-types-basic

An attribute's value is contextually typed by its prop in the props the
element resolves to (`getContextualTypeForJsxAttribute`). A union of props is
first narrowed by the discriminants the attributes write, an omitted optional
discriminant counting as `undefined` (`discriminateContextualTypeByJSXAttributes`).
A spread attribute is contextually typed by the whole props, a body child by
the `children` prop, several overloads by the union of their props, and a
generic component's props are instantiated from the attributes first
(`inferJsxTypeArguments`). The `wrong*` elements show each context is the
right one.
