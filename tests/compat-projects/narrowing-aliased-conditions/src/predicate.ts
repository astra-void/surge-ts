export {};
class Utils {
  static isDefined<T>(value: T): value is NonNullable<T> {
    return value != null;
  }
}
declare function isString(value: unknown): value is string;
class Holder {
  readonly testNumber: number | undefined;
  mutable: number | undefined;
  foo() {
    const isNumber = Utils.isDefined(this.testNumber);
    if (isNumber) {
      const x: number = this.testNumber;
    }
    const isMutableNumber = Utils.isDefined(this.mutable);
    if (isMutableNumber) {
      const y: number = this.mutable;
    }
  }
}
function plain(v: string | number) {
  const ok = isString(v);
  if (ok) {
    const s: string = v;
  } else {
    const n: number = v;
  }
}
