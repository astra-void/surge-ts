class Box<T> {
  #v: T;
  constructor(v: T) { this.#v = v; }
  get value(): T { return this.#v; }
  set value(next: T | string) { this.#v = next as T; }
}
const b = new Box<number>(1);
b.value = "from string";
b.value = 2;
export { b };
