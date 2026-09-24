export {};
class Utils {
  static isDefined<T>(value: T): value is NonNullable<T> {
    return value != null;
  }
}
declare function isDefined<T>(value: T): value is NonNullable<T>;
declare function isArrayOf<T>(value: unknown, sample: T): value is T[];
declare function isString(value: unknown): value is string;

interface Box { inner: { value: string | undefined; count: number | null } }

class Holder {
  readonly testNumber: number | undefined;
  foo(box: Box) {
    if (Utils.isDefined(this.testNumber)) {
      const n: number = this.testNumber;
    }
    if (isDefined(this.testNumber)) {
      const n: number = this.testNumber;
    } else {
      const u: undefined = this.testNumber;
    }
    if (isDefined(box.inner.value)) {
      const s: string = box.inner.value;
    }
    if (isDefined(box.inner.count) && box.inner.count > 1) {
      const c: number = box.inner.count;
    }
    const r = isDefined(box.inner.value) ? box.inner.value : "";
    const t: string = r;
    if (isString(box.inner.value)) {
      const s: string = box.inner.value;
    }
    if (isDefined(box.inner)) {
      const bad: string = box.inner.value;
    }
  }
}

function notTheBase(o: { p: string | number } | undefined) {
  if (o && isString(o.p)) {
    const s: string = o.p;
    const whole: { p: string | number } = o;
  }
}
