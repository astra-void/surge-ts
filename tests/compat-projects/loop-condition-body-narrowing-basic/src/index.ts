declare function next(): string | undefined;
declare function take(value: string): void;

// A loop body starts where its condition held.
function truthy(value: string | undefined): void {
  while (value) {
    take(value);
    value = next();
  }
}
function comparison(value: string | undefined): void {
  while (value !== undefined) {
    take(value);
    value = next();
  }
}
function typeTest(value: string | number): void {
  while (typeof value === "string") {
    take(value);
    break;
  }
}
function classicFor(value: string | undefined): void {
  for (; value; ) {
    take(value);
    value = next();
  }
}
function conditionalReassign(value: string | undefined, flag: boolean): void {
  while (value) {
    take(value);
    if (flag) {
      value = next();
    }
  }
}

// An assignment inside the body still widens what follows it.
function afterAssignment(value: string | undefined): void {
  while (value) {
    value = next();
    take(value);
  }
}
// A `do … while` body runs before the condition is tested.
function doWhile(value: string | undefined): void {
  do {
    take(value);
  } while (value);
}
