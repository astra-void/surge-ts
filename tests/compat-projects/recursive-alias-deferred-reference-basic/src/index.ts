interface Elem { type: string }
interface Nodes {}
type Node1 = string | number | Iterable<Node1> | null;
declare const n1: Node1;
const y1: 0 = n1;
type Awaited1 = Elem | string | Iterable<Node2> | null;
type Node2 = Elem | string | bigint | Iterable<Node2> | boolean | null | undefined | Nodes[keyof Nodes] | Promise<Awaited1>;
declare const n2: Node2;
const y2: 0 = n2;
declare namespace R {
  type ReactNode = Elem | string | Iterable<ReactNode> | null | Promise<AwaitedNode>;
  interface Props { children?: ReactNode }
}
type AwaitedNode = Elem | string | Iterable<R.ReactNode> | null;
declare const n3: R.ReactNode;
const y3: 0 = n3;
declare const p: R.Props;
const y4: 0 = p;
