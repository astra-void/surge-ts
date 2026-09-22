export interface Shared {}
interface Shared {}
namespace Inner {
  export interface Deep {}
  interface Deep {}
}
export namespace Empty {}
namespace Empty {}
export namespace Valued {
  export const a = 1;
}
namespace Valued {
  const b = 1;
}
export class Classy {}
interface Classy {}
export const plain = 1;
export type Plain = 1;

interface Renamed<T> {}
interface Renamed<U> {}
interface Defaulted<T> {}
interface Defaulted<T, U = 1> {}
interface Shrunk<T, U> {}
interface Shrunk<T> {}
class Both<T> {}
interface Both<T> {}
interface Constrained<T extends string> {}
interface Constrained<T extends number> {}
interface Loosened<T extends string> {}
interface Loosened<T> {}
