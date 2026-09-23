class Pair<T extends string, U extends number> {
    constructor(first: T, second: U);
    constructor(flag: boolean);
    constructor(first: unknown, second?: unknown) { }
}
new Pair(true);
new Pair("a", 1);

class Single<T extends string> {
    constructor(count: number);
    constructor(text: T);
    constructor(value: unknown) { }
}
new Single(1);
