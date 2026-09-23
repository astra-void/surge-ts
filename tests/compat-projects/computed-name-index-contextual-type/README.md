# computed-name-index-contextual-type

tsc's `getContextualTypeForObjectLiteralElement`: a member whose computed
name is no literal (`["prefix" + suffix]`, `[index + 1]`) takes its
contextual type from the index signature its key applies to
(`findApplicableIndexInfo`) — a numeric key the number index signature,
falling back to the string one — so the arrow's parameter is typed. Without
a contextual type it is an implicit `any` (TS7006).
