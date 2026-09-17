# loop-assignment-join-basic

tsc types the code after a loop from every edge into it: the end of the body,
carrying what the body assigned, and — unless the body always runs — the state
before the first iteration. surge discarded a function-body loop's assignments
when its scope closed, so a binding narrowed before the loop kept that narrowing
afterwards. A `do … while` body always runs, so only its end state reaches the
code after it (`value` is `number` there). A loop that assigns nothing leaves the
narrowing alone.
