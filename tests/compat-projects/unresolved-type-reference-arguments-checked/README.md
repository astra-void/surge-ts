# unresolved-type-reference-arguments-checked

tsc's `checkTypeReferenceNode` checks a reference's type arguments
(`checkSourceElements(node.TypeArguments())`) before resolving its name, so
the arguments of an unresolved reference, or of one naming a non-generic type
(TS2315), still report their own unresolved names.
