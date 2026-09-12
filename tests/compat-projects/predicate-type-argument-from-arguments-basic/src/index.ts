type Shape = { kind: 'circle'; radius: number } | { kind: 'square'; side: number };

declare function isOfKind<T, K extends string>(
  kind: K,
  value: T,
): value is Extract<T, { kind: K }>;

export function fromAnotherArgument(shape: Shape): number {
  if (isOfKind('circle', shape)) {
    return shape.radius;
  }
  return shape.side;
}

declare function isKindOf<T>(value: T, kind: string): value is Extract<T, { kind: 'circle' }>;

export function fromTheTestedArgumentAlone(shape: Shape): number {
  if (isKindOf(shape, 'circle')) {
    return shape.radius;
  }
  return shape.side;
}

export function theOtherBranchIsTheOtherMember(shape: Shape): number {
  if (isOfKind('circle', shape)) {
    return shape.radius;
  }
  return shape.radius;
}
