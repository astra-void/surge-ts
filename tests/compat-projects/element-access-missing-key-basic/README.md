# element-access-missing-key-basic

tsc's `getPropertyTypeForIndexType` for an element access whose literal key
neither a member nor an index signature answers: under `noImplicitAny` the
whole access is an implicit `any` (TS7053), or TS2576 when a static member of
the class has that name (`C["s"]`). surge reported TS2339 instead — naming the
receiver binding as the missing property for a numeric key.
