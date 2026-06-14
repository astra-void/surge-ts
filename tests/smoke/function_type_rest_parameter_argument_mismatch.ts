type RestHandler = (...values: string[]) => number;

function callRest(fn: RestHandler): void {
  fn(123);
}
