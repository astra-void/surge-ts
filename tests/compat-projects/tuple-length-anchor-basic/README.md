# tuple-length-anchor-basic

An array literal whose length does not fit a tuple type is a mismatch of the
whole value, so tsc reports it as the argument it is (TS2345) or on the
target it initializes or is assigned to, not on the first excess element. A
literal that is too short named its source `unknown[]`. A member assignment
passes its target along, so any whole-value mismatch (a missing property
included) is anchored there.
