# if-surviving-branch-narrowing-basic

tsc's flow node after an `if` joins the branches that can complete. When only
one can — the other returns, throws, `continue`s or `break`s — the code after
the `if` sees exactly what that branch left: its condition narrowing, the
fall-through of an `else if` nested inside it, and anything it assigned.
surge applied fall-through narrowing only to an `if` with no `else`, so after
`if (a) { return } else if (b) { return }` the subject was still the whole
declared type and every use of the remaining member was a false error. When
both branches complete nothing is carried, and a name the surviving branch
*declares* is never mistaken for the outer binding.
