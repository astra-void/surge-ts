class Slot<T> { value!: T }
class Left<T> extends Slot<T> {
    next(): Right<Left<T>> { throw 0 }
}
class Right<T> extends Slot<T> {
    next(): Left<Right<T>> { throw 0 }
}
declare const root: Left<number>;
const bad: string = root.next().value.value;
const missing = root.next().value.missing;
