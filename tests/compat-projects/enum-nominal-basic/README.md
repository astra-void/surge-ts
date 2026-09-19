# enum-nominal-basic

tsc's enum types are nominal (`isEnumTypeRelatedTo`): a member of one enum
never relates to another enum, even where the values coincide, and `E.A`
read off the enum object is the member type `E.A`, not its value. surge
compared the values, so `e = f`, `takesE(F.X)` and `E | F` into `E` were
all accepted.
