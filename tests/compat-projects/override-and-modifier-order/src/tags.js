class Animal {
    speak() {}
}

class Dog extends Animal {
    /** @override */
    speak() {}
    /** @override */
    bark() {}
    /** @override */
    speek() {}
    speak2() {}
}

class Cat {
    /** @override */
    purr() {}
}

class Bird extends Animal {
    speak() {}
}

class Fields {
    /**
     * @readonly
     * @private
     */
    secret = 1;
}

let tagKey = "k";

class TaggedDynamic extends Animal {
    /** @override */
    [tagKey]() {}
}
