# script-typeof-class-member-basic

A script's top-level `var`s are globals the binder declares before any
annotation is resolved, so `typeof x` in a class member's annotation reads
them wherever they are written — in the same script or another one. surge
builds a script class while collecting global signatures, and its members
resolve `typeof` through the checker context, which must see the script
values seeded for that phase. A member typed from the wrong value still
reports (TS2322).
