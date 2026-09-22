# jsx-missing-intrinsic-elements-basic

With no JSX typings in the program at all, every intrinsic element is an
implicit `any` (TS7026), once per element. The report is deliberately limited
to that case: an intrinsic table that exists but did not reach the file is a
resolution gap, not a source error.

tsc also reports a closing tag (`</section>`) as its own element; surge's AST
keeps no closing-tag span, so the fixture pins self-closing elements only.
