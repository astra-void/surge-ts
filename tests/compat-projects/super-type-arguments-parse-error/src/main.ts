class Base<T> {
    constructor(value?: T) {}
}
class Derived extends Base<string> {
    constructor() {
        super<string>();
    }
}
class NoBase {
    constructor() {
        super();
    }
}
const missing: number = "not checked once the program has a parse error";
