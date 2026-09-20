# element-read-dependent-narrowing-basic

An element access on an object receiver reads the member a literal key names
or the applicable index signature, so `obj[key].member` keeps its type instead
of degrading. A tagged template whose tag is an interface with a call
signature is checked like any call, stopping at the first inapplicable
argument.

Typing those reads exposed the dependent narrowing of `const [error, value] =
source` over a union of tuples, which only `if` statements on a named source
applied. It now holds in a conditional expression, an `&&`/`||` chain and a
closure, and for a source that is a call or an `await`. A parameter of the
same name is a different binding.
