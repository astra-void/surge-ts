# mapped-type-member-in-body-basic

`checkGrammarProperty`: a property whose computed name is an `in` expression
(`[k in "a"]`) is a mapped type written among other members, so tsc reports
TS7061 on the first member of the class, interface or type literal and skips
the dynamic-name checks (TS1166/TS1169/TS1170). `isInvalidComputedPropertyName`
then leaves the name's expression unchecked, except on a `get`/`set` accessor.
A real mapped type and literal or entity computed names are fine.
