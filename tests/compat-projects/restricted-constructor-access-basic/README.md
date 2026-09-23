# restricted-constructor-access-basic

Access rules that depend on where a restricted member is reached from:
a protected instance member only through the enclosing class's own instances
(TS2446), `new` on a private/protected constructor outside the class
(TS2673/TS2674), extending a class with a private constructor (TS2675), and
a private or protected member indexed through a type parameter (TS4105).
