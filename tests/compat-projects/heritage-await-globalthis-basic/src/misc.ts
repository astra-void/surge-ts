export interface Shape {
  area(): number;
}

export class Square extends Shape {
  area(): number {
    return 1;
  }
}

export function wait(promise: Promise<number>): number {
  return await promise;
}

export function readGlobal(): unknown {
  return globalThis.missingGlobalMember;
}
