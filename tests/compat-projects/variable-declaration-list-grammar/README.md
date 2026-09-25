# variable-declaration-list-grammar

tsc's parser ends a variable declaration list quietly wherever no binding
follows and the statement could end, so `var;` and `let a = 1, b = 2,;` parse,
and its checker reports the empty list (TS1123) and the trailing comma
(TS1009) on the list itself. A declaration continued on the next line is still
one declaration. The rest of the file is checked as usual.
