type RestHandler = (...values: Missing[]) => number;

function callRest(fn: RestHandler): void {
  const n: number = fn("a", "b");
}
