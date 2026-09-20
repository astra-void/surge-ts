export class Box<T> {
  private value: T;

  constructor(value: T) {
    this.value = value;
  }

  get(): T {
    return this.value;
  }

  swap(next: T): T {
    const previous: T = this.value;
    this.value = next;
    missingInBody();
    return previous;
  }

  static of(value: T): Box<T> {
    return new Box(value);
  }

  static empty: T;

  static make<T>(value: T): Box<T> {
    return new Box(value);
  }

  static read() {
    const held: T = null as never;
    return held;
  }
}

export class Pair<A, B> {
  constructor(
    readonly left: A,
    readonly right: B,
  ) {}

  flip(): Pair<B, A> {
    return new Pair(this.right, this.left);
  }

  static get first(): A {
    return null as never;
  }
}
