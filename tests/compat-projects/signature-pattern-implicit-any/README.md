# signature-pattern-implicit-any

A binding pattern in a signature with no body takes its type from the pattern
alone (`getTypeFromBindingPattern`), which under `noImplicitAny` reports every
element with neither an initializer nor a pattern of its own as TS7031, at its
name. An object rest, an array pattern with nothing but a rest, and a renamed
element (left to the unused-renaming check, TS2842) imply nothing to report;
an initializer inside the pattern is TS2371, as on the parameter itself.
