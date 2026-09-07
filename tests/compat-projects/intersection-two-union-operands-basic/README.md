# intersection-two-union-operands-basic

`(A | B) & C` distributes to `(A & C) | (B & C)`, and surge did that — but only
for a *lone* union operand. tRPC's server-side helper options are
`(queryClient | queryClientConfig) & (external | internal)`: two unions, which
fell through to the undistributed merge that keeps only the first operand's
properties, so every `router` / `ctx` / `client` read was a false `TS2339`.

Every union operand distributes now. The bound moved with it: it is on the
*product* of the arms, so the worst case stays the same number of merges however
the operands split, and anything wider still falls through to the single merge
marked open.

Distribution is also depth-bounded. Each arm's merge peels its operands, and
peeling a deferred intersection reference re-enters the merge — a
self-referential shape in tRPC recursed until the stack gave out the first time
two unions distributed. Past that depth the merge falls back to the single open
form, which is what these shapes got before distribution reached them at all.

The single intentional error is the last function: `router` really is a `string`,
so binding it to a `number` reports.
