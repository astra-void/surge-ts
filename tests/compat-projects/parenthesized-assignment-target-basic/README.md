# parenthesized-assignment-target-basic

tsc reports a value that does not fit on the assignment target as written,
parentheses included (`(x) = ''` is reported at the parenthesis), while a
diagnostic about the name itself stays on the name (`(fixed) = 2`).
