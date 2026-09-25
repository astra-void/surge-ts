export class Box<A = unknown, E = Error> {
  #value: [A, E] = undefined!;
  get(): [A, E] {
    return this.#value;
  }
}
