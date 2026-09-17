type F = (x: number) => number;
declare function take(f: F): void;
declare const flag: boolean;

const declared: F = (x) => "s";

let assigned: F = (x) => x;
assigned = (x) => "t";

take((x) => "u");

const inProperty: { f: F } = { f: (x) => "v" };

const parenthesized: F = (x) => ({ a: 1 });

const conditional: F = (x) => (flag ? "a" : 1);

const objectBody: () => { p: number } = () => ({ p: "s" });

const annotatedParameter: F = (x: number) => "w";

const blockBody: F = (x) => {
  return "y";
};

declare const items: { name?: string }[];
const found = items.find((item) => item.name);
const index = items.findIndex((item) => item.name);
