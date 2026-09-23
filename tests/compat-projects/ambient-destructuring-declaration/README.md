# ambient-destructuring-declaration

An ambient destructuring declaration has no initializer; its elements take
their types from the annotation (`declare let { count, label }: {…}`,
`declare const [flag, total]: […]`), so the names are declared globals and
a mismatched use of one is TS2322.
