# catch-annotation-resolved-type

A catch variable's annotation must resolve to `any` or `unknown` (tsc's
`checkTryStatement`), so an alias of either is accepted. Anything else is
TS1196 on the annotation, and the variable then has the error type rather than
the written one, so reads of it report nothing further.
