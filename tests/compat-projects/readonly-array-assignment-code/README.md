# readonly-array-assignment-code

tsc's relation reports a `readonly` array or tuple written to a mutable array
or tuple as TS4104 on its own, with no TS2322 head — for an assignment
expression exactly as for a declaration.
