export class Counter {
  count = 0;
  static total = 0;
  doubled = count * 2;

  increment() {
    const viaArrow = () => count;
    const viaFunction = function () {
      return count;
    };
    return viaArrow() + viaFunction() + total;
  }

  static reset() {
    return count + total;
  }
}
