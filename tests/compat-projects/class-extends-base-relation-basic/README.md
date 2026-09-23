# class-extends-base-relation-basic

tsc's `checkClassLikeDeclaration` relates the class to its base: a member
modifier mismatch the per-member walk (TS2416) does not attribute is TS2415
on the class name, and the static side is related separately (TS2417).
Widening a protected member and redeclaring a compatible one are fine.
