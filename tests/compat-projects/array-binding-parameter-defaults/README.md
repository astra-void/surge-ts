# array-binding-parameter-defaults

tsc's `getTypeFromBindingElement` types a binding element with a default from
that default, so only an element without one is TS7031 when the parameter has
no type. With an array literal initializer the parameter's type comes from the
literal, padded to the pattern's length (`checkArrayLiteral` under a binding
pattern contextual type), and a padded element without a default is the
implicit `any` `reportErrorsFromWidening` reports.
