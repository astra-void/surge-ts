# callback-argument-signature-basic

A contextually typed function whose returns do not fit is reported by tsc as
one mismatch of the whole signature. As a call argument that is an argument
error (TS2345, `Argument of type '(value: number) => string' …`), and it is
the call's first inapplicable argument, so later ones are not reported.
Anywhere else it stays TS2322. surge reported TS2322 in both.
