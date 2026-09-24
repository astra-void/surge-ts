# decorator-expressions-checked

tsc checks the expression of every decorator on a declaration that can be
decorated (`checkDecorators`): what it names, and, outside a function it
defers to, a reference to the decorated class itself or to a class declared
later (TS2449). A parameter decorator is checked only under
`experimentalDecorators`, and a function written as the decorator is
contextually typed by the decorator call.
