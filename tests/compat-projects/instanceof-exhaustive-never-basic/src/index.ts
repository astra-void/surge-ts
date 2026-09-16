class Shape {
  area = 0;
}
class Square extends Shape {
  side = 1;
}

// Once every constructor is ruled out, what is left is `never`.
function classify(value: Date | RegExp): string {
  if (value instanceof Date) {
    return "date";
  }
  if (value instanceof RegExp) {
    return "pattern";
  }
  const unreachable: never = value;
  return unreachable;
}

// An instance of the constructor, or of a subclass, cannot reach `else`.
function sameClass(value: Date): void {
  if (!(value instanceof Date)) {
    const impossible: never = value;
  }
}
function subclass(value: Square): void {
  if (value instanceof Shape) {
  } else {
    const impossible: never = value;
  }
}

// A base class instance can still fail a subclass test.
function baseClass(value: Shape): void {
  if (value instanceof Square) {
  } else {
    const possible: never = value;
  }
}
