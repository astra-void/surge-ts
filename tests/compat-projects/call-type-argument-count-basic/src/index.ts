declare function one<T>(x: T): T;
declare function constrained<T extends string>(x: T): T;
declare function plain(x: number): void;
declare function withDefault<N extends number = 1>(n?: N): N;
declare const untyped: any;

one<string, number>("a");
constrained<number>(1);
plain<string>(1);
untyped<string>(1);
withDefault<"x">();
withDefault();
one<string>("fine");

class Box<T> {
  constructor(public value: T) {}
}
class Plain {}
class Pair<A, B = string> {}
new Box<string, number>("a");
new Box<string>("a");
new Plain<string>();
new Pair<number>();
new Pair<1, 2, 3>();
new Map<string, number>();
