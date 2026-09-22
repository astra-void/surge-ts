# interface-method-overload-basic

An interface method declared more than once is an overload group. surge folded
the declarations into one permissive signature and dropped the members, so a
call was checked against the fold alone: `runner.run(true)` was silent (the
fold's first parameter is neither `string` nor `number`), and a call no
overload accepts could never be TS2769. The members now ride along with the
fold, as the interface's own call signatures already did: with one candidate
of the call's arity its mismatch is a TS2345 against that candidate, with
several it is TS2769 — anchored, as tsc's `reportCallResolutionErrors` does, at
the first argument the *last* candidate rejects.
