class PropertyThenGetter {
    x = 1;
    get x() { return 1; }
}

class SetterThenProperty {
    set x(value: number) {}
    x = 1;
}

class GetSetThenProperty {
    get x() { return 1; }
    set x(value: number) {}
    x = 1;
}

class GetSetPair {
    get x() { return 1; }
    set x(value: number) {}
}

class TwoProperties {
    x = 1;
    x = 2;
}

class StaticAndInstance {
    static x = 1;
    get x() { return 1; }
}

class PrivateNames {
    #x = 1;
    get #x() { return 1; }
}

class AutoAccessor {
    accessor x = 1;
    x = 2;
}

class ParameterProperty {
    y = 1;
    constructor(public y: number) {}
}

class PrivateParameterProperty {
    y = 1;
    constructor(private y: number) {}
}

class ReadonlyParameterProperty {
    constructor(readonly z: number, protected w: number) {}
    z = 1;
    w = 1;
}

class PlainParameter {
    y = 1;
    constructor(y: number) {}
}

export {
    PropertyThenGetter, SetterThenProperty, GetSetThenProperty, GetSetPair, TwoProperties,
    StaticAndInstance, PrivateNames, AutoAccessor, ParameterProperty, PrivateParameterProperty,
    ReadonlyParameterProperty, PlainParameter,
};
