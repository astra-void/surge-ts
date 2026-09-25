# decorator-expressions-checked

tsc checks the expression of every decorator on a declaration that can be
decorated (`checkDecorators`): what it names, and, outside a function it
defers to, a reference to the decorated class itself or to a class declared
later (TS2449). A parameter decorator is checked only under
`experimentalDecorators`, and a function written as the decorator is
contextually typed by the decorator call.
Also the decorator grammar: an expression that must be parenthesized
(TS1497), a decorator that takes too few parameters to be applied uncalled
(TS1329), a decorator no signature of which accepts the arguments the
runtime passes (TS1238), and a decorator on a declaration that cannot be
decorated (TS1206).
