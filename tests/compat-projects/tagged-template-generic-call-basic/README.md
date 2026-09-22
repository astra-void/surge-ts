# tagged-template-generic-call-basic

`` tag`a${x}b` `` is a call of `tag` (tsc's `resolveTaggedTemplateExpression`):
its effective arguments are the template's `TemplateStringsArray` and then
each substitution, and the tag's type arguments are the call's. So a
substitution is contextually typed by the tag's parameter, arity and
argument types are checked, generic tags infer from the substitutions, and
overloads resolve. surge lowered a tagged template as a plain template
literal with the tag as its first piece: every callback substitution was a
false TS7006 and only a narrow non-generic signature was ever checked.

A non-generic, single call signature also answers a call's result type
during inference, so `f("")[0]` reads the result of calling a callable
interface.
