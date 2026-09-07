interface Ty {
  flags: number;
  isUnion(): this is UnionTy;
  isIntersection(): this is InterTy;
}
interface UnionTy extends Ty {
  types: Ty[];
}
interface InterTy extends Ty {
  types: Ty[];
}

declare function isPrimitive(type: Ty): boolean;
declare function takeUnion(union: UnionTy): void;

export function unwrapBrand(type: Ty): Ty {
  if (!type.isIntersection()) {
    return type;
  }
  const primitives = type.types.filter(isPrimitive);
  return primitives.length > 0 ? type : type;
}

export function inIfBranch(type: Ty): number {
  if (type.isUnion()) {
    return type.types.length;
  }
  return 0;
}

export function asArgument(type: Ty): void {
  if (type.isUnion()) {
    takeUnion(type);
  }
}

export function inAndChain(type: Ty): boolean {
  return type.isUnion() && type.types.every((member) => member.flags > 0);
}

export function elseBranchIsUnnarrowed(type: Ty): Ty[] {
  if (type.isUnion()) {
    return type.types;
  }
  return type.types;
}
