# module-condition-comparison-basic

A module-scope `if` condition is an ordinary expression: tsc reports an
unintentional comparison (TS2367) or a bad relational operand (TS2365) in it.
surge evaluated the condition for narrowing only and discarded its
diagnostics, and a comparison written as a bare expression statement lost
its location.
