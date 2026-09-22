# defaulted-parameter-optionality-basic

tsc's `addOptionality`: a parameter with an initializer accepts `undefined`
from its callers wherever it stands. A trailing one is simply optional; one a
required parameter follows — the Redux `reducer(state = initial, action)`
shape — carries `undefined` in its type, so `reducer(undefined, action)` is
valid and the signature reads `(state: State | undefined, action) => State`.
surge gave such a parameter its bare type, a false TS2345 on every such call.
Inside the body the initializer fills the gap, so the binding is `T` even when
the annotation itself wrote `T | undefined`.

Only the *initial* type drops `undefined`: a parameter annotated
`T | undefined` stays declared that way, so `x = undefined` in the body is a
valid write. An initializer that can itself be `undefined` fills nothing.
