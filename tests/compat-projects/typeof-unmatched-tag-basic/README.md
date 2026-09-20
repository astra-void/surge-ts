# typeof-unmatched-tag-basic

A `typeof` test against a tag no value reports (`"Object"`, already
TS2367) still narrows: the two branches split the subject by
primitiveness, since nothing primitive can hide behind an unrecognized
tag and nothing else can be ruled out by one. surge left the union
standing in both branches.
