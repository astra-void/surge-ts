interface Box<T> {
  value: T;
}
interface Flipped<A, B> {
  value: B;
  other: A;
}
declare function unbox<T>(box: Box<T>): T;
declare const flipped: Flipped<string, number>;
const y1: 0 = unbox(flipped);

interface Holder<T> {
  get(): T;
}
interface Listed<A> {
  get(): A[];
}
declare function unhold<T>(holder: Holder<T>): T;
declare const listed: Listed<number>;
const y2: 0 = unhold(listed);

declare const box: Box<boolean>;
const y3: 0 = unbox(box);
