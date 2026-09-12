# generic-callback-argument-inference-basic

Three inference gaps that together made vitest's `vi.fn(…)` unusable across
TanStack Query's tests, each reproduced against a mock-shaped generic:

- A *generic function* passed as the argument carries its own type parameters as
  placeholders. The call refused it as a type-argument candidate, left `Mock<T>`
  uninstantiated, and the result read as not callable.
- `.then`/`.catch` on a promise-returning call is answered by the checking pass
  but was degraded by the inference-only pass, so a callback whose body is a
  promise chain inferred nothing.
- `'data' + String(value)` inferred `number` in that same pass whenever the
  right operand was one it could not type, so the callback's return type
  disagreed with what the checking pass saw.
