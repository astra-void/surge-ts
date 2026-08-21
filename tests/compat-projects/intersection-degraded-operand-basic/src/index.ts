interface Shape {
  path?: number;
}
interface Other {
  extra: number;
}

// `T` binds to the degradation sentinel during signature mapping, so the
// intersection drops it and `Other` survives alone. A closed survivor reports
// every member that lived on `T`'s constraint as missing.
export function namedSurvivor<T extends Shape>(o: T & Other): number | undefined {
  return o.path;
}

// The inline survivor was already opened; both must behave the same.
export function inlineSurvivor<T extends Shape>(o: T & { extra: number }): number | undefined {
  return o.path;
}

// A member on the survivor itself still resolves.
export function survivorMember<T extends Shape>(o: T & Other): number {
  return o.extra;
}

// No degraded operand: an unknown member is still an error.
declare const concrete: Other;
export const stillReported = concrete.path;
