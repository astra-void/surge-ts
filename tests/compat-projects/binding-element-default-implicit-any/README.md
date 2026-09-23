# binding-element-default-implicit-any

In an unannotated parameter's object pattern, an element with a default is
typed from it (tsc's `getTypeFromBindingElement`); only an element without one
is an implicit `any` (TS7031), at any nesting depth.
