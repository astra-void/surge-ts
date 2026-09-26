export class Counter {
  #count = 0;

  increment() {
    return ++this.#count;
  }
}
