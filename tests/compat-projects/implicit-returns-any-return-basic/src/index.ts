declare const anyVal: any;
declare const unknownVal: unknown;
declare const cond: boolean;

export function retAny() {
  if (cond) return anyVal;
}

export function retUnknown() {
  if (cond) return unknownVal;
}

export function retNumber() {
  if (cond) return 1;
}

export class C {
  p: Promise<void> | null = null;

  async openVoid() {
    if (this.p) return this.p;
  }

  async openNumber() {
    if (this.p) return 1;
  }
}
