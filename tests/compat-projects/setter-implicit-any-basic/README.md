# setter-implicit-any-basic

Under noImplicitAny, an accessor pair with no getter return annotation, no
getter body and no setter parameter annotation is an implicit any property,
reported on the setter (TS7032, getTypeOfAccessors) next to the parameter's
own TS7006.
