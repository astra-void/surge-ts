# class-heritage-non-constructor-basic

An `extends` value without construct signatures is TS2507
(`getBaseConstructorTypeOfClass`); a primitive keyword in `extends` or
`implements` is TS2863/TS2864 (`checkAndReportErrorForUsingTypeAsValue`,
`checkClassLikeDeclaration`). `any`, a class and a constructor type are fine.
