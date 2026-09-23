export function relay<T extends any[]>(t: T) {
  (function (...x) {
    return x;
  })(...t);
  (function (a, ...x) {
    return [a.toFixed(), x];
  })(1, ...t);
  (function (a, b, ...x) {
    return [a, b, x];
  })(1, 2, ...t);
}
