interface HasLength {
  length: number;
}

function measure<T extends HasLength>(value: T): number {
  return value.length;
}

measure(5);
measure(true);
measure("text");
measure([1, 2]);
measure({ length: 3 });

function echo<T extends HasLength>(value: T): T {
  return value;
}
const echoed = echo(3);
