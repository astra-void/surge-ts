# logical-assignment-narrowing-basic

`x ??= v`, `x ||= v` and `x &&= v` assign, so the code after them sees the
assigned type: `bag.patterns ??= new Set()` makes `bag.patterns` a `Set`, and
`bag.allOf = bag.allOf ?? []` an array. surge dropped every compound assignment
at parse time and never narrowed an optional property through an assignment
either (the narrowed type equalled the written one, so it counted as no
change). The last function pins that an unassigned optional member still
reports.
