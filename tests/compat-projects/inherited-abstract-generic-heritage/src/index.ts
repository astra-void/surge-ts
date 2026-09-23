export {};

abstract class A<T> {
  t!: T;
  abstract foo(): T;
  abstract bar(t: T): void;
}

abstract class B<T> extends A<T> {}

class C<T> extends A<T> {}

class D extends A<number> {}

class E<T> extends A<T> {
  foo() {
    return this.t;
  }
}

class F<T> extends A<T> {
  bar(t: T) {}
}

class G<T> extends A<T> {
  foo() {
    return this.t;
  }
  bar(t: T) {}
}

class H<K, V> extends A<Map<K, V>> {
  bar(t: Map<K, V>) {}
}
