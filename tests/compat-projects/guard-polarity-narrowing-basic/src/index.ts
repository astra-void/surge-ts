export function afterOrChainExit(error: unknown): boolean {
  if (typeof error !== 'object' || error === null || !('digest' in error)) {
    return false;
  }
  return error.digest === 'NEXT_NOT_FOUND';
}

export function insideOrChain(error: unknown): boolean {
  if (
    typeof error !== 'object' ||
    error === null ||
    !('digest' in error) ||
    typeof error.digest !== 'string'
  ) {
    return false;
  }
  return error.digest.length > 0;
}

declare function isObject(value: unknown): value is Record<string, unknown>;

export function comparedAgainstFalse(value: unknown): boolean {
  if (isObject(value) === false) {
    return false;
  }
  const ctor = value.constructor;
  return ctor !== undefined;
}

export function unguardedStaysUnknown(error: unknown): unknown {
  return error.digest;
}
