# tagged-template-call-basic

A tagged template is a call: each interpolation is an argument after the
strings array (TS2345 on a mismatch), and the expression has the tag's return
type. An interpolated symbol is still TS2731. surge evaluated the tag without
calling it, so the arguments went unchecked and the result was unknown.
Generic and overloaded tags are not related yet.
