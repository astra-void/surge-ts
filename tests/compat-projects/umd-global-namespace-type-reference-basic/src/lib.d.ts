export = Lib;
export as namespace Lib;

declare namespace Lib {
  interface Ctx<T> {
    value: T;
  }
  type Node = string | number | Iterable<Node> | null;
  namespace JSX {
    interface Element {
      tag: string;
    }
  }
}
