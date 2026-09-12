# typeof-function-member-generic-call-basic

A property typed `typeof fn`, where `fn` is a generic declaration, is called
through the property path — which had no generic instantiation at all. The
member annotation resolved eagerly to the declaration's bare function type,
dropping its parsed signature, and `check_function_type_call` takes none. For
vitest this meant every `vi.fn()` returned `Mock<T>` with `T` unbound; peeling
that could not resolve the conditional operand carrying the call signature, and
the mock was neither callable nor assignable to any callback (63 diagnostics in
the tanstack-query corpus).

Three pieces close it:

- the `typeof` arm keeps the declaration on the function handle (an opaque
  slot on `FunctionType`, outside identity and equality), unless the
  declaration is an overload group — a group keeps only its first signature as
  template, and instantiating `vi.spyOn(obj, "method")` against the `"get"`
  accessor overload bound the wrong parameters (146 false TS2345 when tried);
- the property-call path instantiates through that signature exactly as a
  symbol call does;
- a type parameter with no inference source at all — no supplied argument
  mentions it, no contextual return type — falls back to its declared default,
  and only when that completes the binding. Defaulting a parameter that *did*
  have a source, or leaving another one unbound, turned inference gaps into
  concrete wrong types (zod's `hash(alg, { enc: "base64" })` against `Enc =
  "hex"`, ofetch's `$fetch(url, options)` against `R = "json"`).

Not pinned here: inferring `T` from an arrow *argument* (`vi.fn((n: number) =>
String(n))`) still yields a permissive result rather than the arrow's type on
both paths, so the typed call is not covered.
