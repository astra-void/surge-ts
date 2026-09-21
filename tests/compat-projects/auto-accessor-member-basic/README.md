# auto-accessor-member-basic

An auto-accessor (`accessor count: number = 0`) is a get/set pair over a
private slot and, for typing, the property it looks like. The class-member
parser dropped it, so every read and write of the member was a false TS2339.
One with neither a type nor an initializer is an implicit `any` (TS7008)
unless the constructor assigns it, exactly like a plain property.
