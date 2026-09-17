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
