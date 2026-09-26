export class A {
  #secret = 1;
  public #shown = 2;
  static read(a: A) {
    return a.#secret;
  }
  drop() {
    delete this.#secret;
  }
  nested() {
    class Inner {
      #secret = "inner";
      peek(a: A) {
        return a.#secret;
      }
    }
    return Inner;
  }
}

export function outside(a: A) {
  return a.#secret;
}
