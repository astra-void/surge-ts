# ambient-implicit-any

Nothing types an ambient variable written with neither a type nor an
initializer, so under `noImplicitAny` tsc reports it as an implicit `any`
(TS7005) wherever the declaration is ambient: a `declare` statement, an
ambient namespace or global block, or a declaration file. A private member of
an ambient class reports no implicit `any` (TS7008) at all.
