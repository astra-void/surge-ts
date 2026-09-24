# with-statement-body-unchecked

tsc's `checkWithStatement` reports the statement itself (TS2410, and TS1101
where it is strict) and checks its object, but never checks its body, so a
misplaced namespace, `export` or label inside a `with` block reports nothing.
The namespace after the block is fine.
