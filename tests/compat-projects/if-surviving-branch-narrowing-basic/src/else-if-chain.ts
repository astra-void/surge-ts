type Shape =
  | { kind: "circle"; radius: number }
  | { kind: "square"; side: number }
  | { kind: "line"; length: number };

export function chain(shape: Shape): number {
  if (shape.kind === "circle") {
    return shape.radius;
  } else if (shape.kind === "square") {
    return shape.side;
  }
  return shape.length;
}

export function chainNotEqual(shape: Shape): number {
  if (shape.kind === "circle") {
    return shape.radius;
  } else if (shape.kind !== "line") {
    return shape.side;
  }
  return shape.length;
}

export function valueChain(value: "a" | "b" | "c"): "c" {
  if (value === "a") {
    throw new Error("a");
  } else if (value === "b") {
    throw new Error("b");
  }
  return value;
}

export function typeofChain(value: string | number | boolean): boolean {
  if (typeof value === "string") {
    return true;
  } else if (typeof value === "number") {
    return false;
  }
  return value;
}

export function fourWay(value: "a" | "b" | "c" | "d"): "d" {
  if (value === "a") throw new Error("a");
  else if (value === "b") throw new Error("b");
  else if (value === "c") throw new Error("c");
  return value;
}

export function loopChain(items: (string | number | boolean)[]): number {
  let total = 0;
  for (const item of items) {
    if (typeof item === "boolean") {
      continue;
    } else if (typeof item === "string") {
      continue;
    }
    total += item;
  }
  return total;
}

export function stillWide(value: "a" | "b" | "c"): "c" {
  if (value === "a") {
    throw new Error("a");
  } else if (typeof value === "number") {
    throw new Error("never");
  }
  return value;
}
