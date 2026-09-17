declare const flag: boolean;
declare function take(x: number): void;

const declared: number = flag ? 1 : "s";

let assigned: number = 0;
assigned = flag ? 2 : "t";

take(flag ? 1 : "s");

const property: { p: number } = { p: flag ? 1 : "s" };

const bothBranches: number = flag ? "a" : "b";

const objectBranches: { p: number } = flag ? { p: "s" } : { p: 1 };

const nested: number = flag ? (flag ? 1 : "x") : 2;

function returned(): number {
  return flag ? 1 : "s";
}

function returnedNested(): number {
  return flag ? (flag ? "a" : 1) : "b";
}

function returnedObjects(): { p: number } {
  return flag ? { p: "s" } : { p: 1 };
}

const annotatedArrow = (): number => (flag ? "x" : 1);
