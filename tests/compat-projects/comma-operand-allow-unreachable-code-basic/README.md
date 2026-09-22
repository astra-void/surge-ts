# comma-operand-allow-unreachable-code-basic

TS2695 (a side-effect-free left operand of `,`) is reported only when
`allowUnreachableCode` is not explicitly `true` (`checkBinaryLikeExpression`).
surge treated the option as a no-op and reported it regardless.
