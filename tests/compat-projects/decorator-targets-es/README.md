# decorator-targets-es

tsc's `nodeCanBeDecorated` without `experimentalDecorators` (ES decorators):
class expressions and their members can be decorated, including private ones,
but abstract and `declare` fields cannot, and no parameter can (each rejected
parameter's first decorator is TS1206). Paired with
`decorator-targets-legacy`.
