// A binding whose module did not resolve takes tsc's error type, so a callback
// passed to a call on it has no contextual type and its parameters are
// implicitly `any`.
import { procedure, register } from 'module-that-does-not-exist';

export const direct = procedure.query((opts) => opts);
export const destructured = procedure.query(({ input }) => input);
export const chained = procedure.input(1).query(({ input }) => input);
export const nested = register({
  greeting: procedure.input(1).query(({ input }) => input),
});
