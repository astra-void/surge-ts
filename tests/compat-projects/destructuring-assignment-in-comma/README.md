# destructuring-assignment-in-comma

A destructuring assignment that is one operand of a comma expression
statement (`[a] = [1], f();`, `f(), { b } = …;`) assigns its targets in
evaluation order like one standing alone, so reading them after the
statement is not TS2454. A variable no operand assigns still is.
