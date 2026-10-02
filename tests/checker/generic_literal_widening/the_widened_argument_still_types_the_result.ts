// The widened argument still types the result: assigning it elsewhere reports.
declare function subject<T>(initial: T): { next: (v: T) => void; get: () => T };
export function f() {
const value = subject(1);
value.next("nope");
}
