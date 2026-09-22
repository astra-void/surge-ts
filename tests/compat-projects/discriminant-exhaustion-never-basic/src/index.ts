type Shape =
  | { kind: "circle"; radius: number }
  | { kind: "square"; side: number }
  | { kind: "line"; length: number };

export function bySwitch(shape: Shape): number {
  switch (shape.kind) {
    case "circle":
      return shape.radius;
    case "square":
      return shape.side;
    case "line":
      return shape.length;
    default: {
      const unreachable: never = shape;
      return unreachable;
    }
  }
}

export function byIfChain(shape: Shape): number {
  if (shape.kind === "circle") return shape.radius;
  if (shape.kind === "square") return shape.side;
  if (shape.kind === "line") return shape.length;
  const unreachable: never = shape;
  return unreachable;
}

export function byValue(direction: "up" | "down"): number {
  switch (direction) {
    case "up":
      return 1;
    case "down":
      return -1;
    default: {
      const unreachable: never = direction;
      return unreachable;
    }
  }
}

export function byNumber(level: 1 | 2): string {
  if (level === 1) return "one";
  if (level === 2) return "two";
  const unreachable: never = level;
  return unreachable;
}

export function skipsMember(shape: Shape): number {
  switch (shape.kind) {
    case "circle":
      return shape.radius;
    case "square":
      return shape.side;
    default: {
      const stillLine: never = shape;
      return stillLine;
    }
  }
}

type Wide = { mode: "read" | "write"; size: number };

export function literalUnionDiscriminant(wide: Wide): number {
  if (wide.mode === "read") return wide.size;
  const stillWrite: never = wide;
  return stillWrite;
}

export function valueNotExhausted(direction: "up" | "down" | "left"): number {
  if (direction === "up" || direction === "down") return 0;
  const stillLeft: never = direction;
  return stillLeft;
}
