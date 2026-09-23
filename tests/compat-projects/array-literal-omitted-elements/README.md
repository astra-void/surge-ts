# array-literal-omitted-elements

An omitted element (`[42, , true]`) is an element of type `undefined`; it keeps
every later element at its position, so `[42, , true]` fits
`[number, string?, boolean?]` and `[1, , 3]` is `(number | undefined)[]`. tsc
never elaborates a mismatch into an omitted element: one it causes is reported
on the whole literal, at the declaration. A mismatched element after a hole is
still reported on that element.
