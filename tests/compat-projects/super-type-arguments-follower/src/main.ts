class Base<T> {
    method(): T | undefined {
        return undefined;
    }
}
class Derived extends Base<number> {
    constructor() {
        super<number> `hello`;
    }
    member() {
        return super<number>.method();
    }
}
export const gated: number = "not reported once the program has a parse error";
