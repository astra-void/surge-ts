declare const cond: boolean;

// `void` anywhere in the return type exempts the body entirely, with or
// without a value return.
export function voidUnionWithReturn(): number | void {
  if (cond) {
    return 1;
  }
}

export function voidUnionWithoutReturn(): number | void {}

export function bareVoid(): void {}

// A union carrying `undefined` is not exempt from TS2355 — only from TS2366.
export function undefinedUnionWithoutReturn(): number | undefined {}

export function undefinedUnionWithReturn(): number | undefined {
  if (cond) {
    return 1;
  }
}

// A reachable end point under a `never` annotation is TS2534, not TS2355.
export function neverWithReachableEnd(): never {
  if (cond) {
  }
}

// A written `unknown` is reported; it is not one of the exempt annotations.
export function unknownWithoutReturn(): unknown {}

export function anyWithoutReturn(): any {}

export function undefinedWithoutReturn(): undefined {}

// The plain cases must keep reporting.
export function numberWithoutReturn(): number {}

export function numberWithPartialReturn(): number {
  if (cond) {
    return 1;
  }
}
