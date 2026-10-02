// A block comment is not a `//` comment: tsc's `isCommentOrBlankLine` stops at
// it, so the directive above it applies to that line and no further.
export function f() {
  // @ts-expect-error - intentional
  /* a block comment */
  const a: number = "s";
  return a;
}
