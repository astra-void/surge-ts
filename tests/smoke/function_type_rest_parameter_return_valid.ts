type RestHandler = (...values: string[]) => number;

function callRest(fn: RestHandler): void {
  const n: number = fn("a", "b");
}
