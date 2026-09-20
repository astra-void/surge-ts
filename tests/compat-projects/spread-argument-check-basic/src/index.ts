declare function all(a?: number, b?: number): void;
declare function prefix(s: string, a?: number, b?: number): void;
declare function rest(s: string, a?: number, b?: number, ...rest: number[]): void;
declare function normal(s: string): void;
declare function tupleRest(s: string, ...rest: [number, boolean] | [string]): void;
declare const ns: number[];
declare const strings: string[];
declare const tuple: [number, string];
declare const pair: [number, boolean];

export function spreads() {
  all(...ns);
  all(...strings);
  all(...tuple);
  prefix("b", ...strings);
  prefix("c", ...tuple);
  rest("e", ...strings);
  rest("f", ...tuple);
  rest("g", 1, 2, ...ns);
  prefix(...ns);
  normal("h", ...ns);
  tupleRest("i", ...pair);
}

(function (a, b) {
  const first: number = a;
  const second: string = b;
  return [first, second];
})(...tuple);
