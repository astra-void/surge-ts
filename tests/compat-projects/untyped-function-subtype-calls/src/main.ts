interface Handler extends Function {
    id: number;
}
declare const handler: Handler;
declare const plain: Function;

export const called = handler(1, "two");
export const typed = handler<number>(1);
export const plainTyped = plain<string>();

class Derived extends Function { }
declare const derived: Derived;
export const fromClass = derived();

interface NotFunction {
    prototype: any;
    length: number;
}
declare const notFunction: NotFunction;
export const notCallable = notFunction();
