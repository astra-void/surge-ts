# rest-element-property-name-basic

A rest element in an object binding pattern cannot have a property name
(`{ ...a: b }`) — TS2566 on the name, per `checkGrammarBindingElement`. oxc
reports the same failure at the annotation it parsed; the grammar pass claims
the code and anchors it where tsc does.
