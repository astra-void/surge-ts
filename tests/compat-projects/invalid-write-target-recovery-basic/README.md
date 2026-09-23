# invalid-write-target-recovery-basic

The write targets and rest positions of `invalid-write-target-basic`, many to
one file: tsc reports each from its grammar checks and keeps checking the
file, so every instance is reported (TS2364, TS2779, TS2357, TS2777, TS1014,
TS2462) along with the ordinary type error at the end. A rest parameter that
is not last is an ordinary parameter for calls.
