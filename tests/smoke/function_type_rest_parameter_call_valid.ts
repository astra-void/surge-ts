type RestHandler = (...values: string[]) => number;

function callRest(fn: RestHandler): void {
  fn("a", "b");
  fn();
}
