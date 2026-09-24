interface A<T> {
    x: T;
}
interface A<U> {
    y: U;
}
declare const a: A<string, number>;
const fromFirst: string = a.x;
const fromSecond: number = a.y;
const wrong: string = a.y;
