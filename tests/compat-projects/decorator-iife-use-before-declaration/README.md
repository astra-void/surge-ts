# decorator-iife-use-before-declaration

A class is used before its declaration (TS2449) wherever the use runs while
the class is still being defined. A function body usually runs later, but an
immediately invoked one runs where it is written
(`isUsedInFunctionOrInstanceProperty` walks past an IIFE), so a decorator
that calls one reaches the class too early.
