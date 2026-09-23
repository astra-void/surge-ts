export {};
class C {
  #m() {}
  set #s(v: number) {}
  #f = 1;
  static #sm() {}
  g(other: C) {
    this.#m = () => {};
    this.#s = 1;
    this.#s;
    this.#f = 2;
    other.#m = () => {};
    this.#s += 1;
    C.#sm = () => {};
    [this.#m] = [() => {}];
  }
}
