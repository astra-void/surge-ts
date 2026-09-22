# regexp-flag-conflict-basic

A regular expression with both the `u` and `v` flags is a scanner error in
tsc (TS1502), reported at the second flag. As a syntax error it stops tsc
checking the program, so it gets a program to itself.
