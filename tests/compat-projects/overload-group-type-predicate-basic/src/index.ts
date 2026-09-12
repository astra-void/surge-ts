type Shape = { kind: 'circle'; radius: number } | { kind: 'square'; side: number };

declare function isCircle(shape: Shape): shape is { kind: 'circle'; radius: number };

export function fromASingleSignature(shape: Shape): number {
  if (isCircle(shape)) {
    return shape.radius;
  }
  return shape.side;
}

declare function match(pattern: string): (shape: Shape) => boolean;
declare function match(
  pattern: string,
  shape: Shape,
): shape is { kind: 'circle'; radius: number };

export function fromALaterOverload(shape: Shape): number {
  if (match('circle', shape)) {
    return shape.radius;
  }
  return shape.side;
}

export function theOtherOverloadStillReturnsItsOwnType(shape: Shape): boolean {
  const test = match('circle');
  return test(shape);
}

export function theFalseBranchIsTheOtherMember(shape: Shape): number {
  if (match('circle', shape)) {
    return shape.radius;
  }
  return shape.radius;
}
