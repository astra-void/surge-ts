# circular-type-alias-basic

tsc reports a type alias whose resolution reaches itself before any deferred
position (TS2456): through union and intersection members, `keyof`, indexed
access, a conditional's check or extends type, template literal spans,
parentheses or another alias. Object members, signatures, array and tuple
elements, conditional branches and other references' type arguments are
deferred, and an alias that only leads into a cycle is not on it. surge
reported its own `surge::type-alias-cycle` under the native profile only.
