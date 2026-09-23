# inference-callback-parameter-fixing

How tsc types a context-sensitive callback argument during inference
(`contextuallyCheckFunctionExpressionOrObjectLiteralMethod`):

- `inferFromAnnotatedParametersAndReturn` runs first: each annotated
  parameter but a rest one infers to the contextual parameter type at its
  position, and an annotated return type to the contextual return type, so
  `testRest((t1: D, t2, t3) => …)` has `T = D` before `t2` is typed.
- Typing an unannotated parameter from the contextual signature fixes every
  type parameter that parameter's contextual type names. A parameter with no
  candidate yet is fixed at `getInferredType` without one: the contextual
  return type's inference, else its default, else its constraint, else
  `unknown`. So `testRest((t1, t2) => {})` fixes `T` at the constraint `C`,
  and the result is not assignable to `D` (TS2741).

surge reads the contextual return type only through a generic-instantiation
return annotation; under a contextual type, a parameter another return shape
names is left unfixed. The intentional error is a tsc error too.
