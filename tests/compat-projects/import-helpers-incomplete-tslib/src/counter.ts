export class Counter {
  #count = 0;

  increment() {
    this.#count += 1;
    return this.#count;
  }

  async reset() {
    this.#count = 0;
  }
}
