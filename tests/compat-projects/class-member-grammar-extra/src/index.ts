abstract class Consecutive {
    abstract foo(): number;
    abstract foo(): string;
    x: number = 0;
    abstract foo(): number;
}

abstract class WithBody {
    abstract method() { }
}

class ConstructorAccessors {
    get constructor() { return 1; }
    set constructor(value) { }
}

class ConstructorOverloads {
    public constructor(a: boolean)
    protected constructor(a: number)
    private constructor(a?: unknown) { }
}

export { };
