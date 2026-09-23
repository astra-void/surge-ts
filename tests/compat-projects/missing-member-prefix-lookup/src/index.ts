export {};

class InstanceIndex {
  [key: string]: unknown;
  read() {
    return missingOne;
  }
}

class StaticIndex {
  static [key: string]: unknown;
  read() {
    return missingTwo;
  }
}

class GenericStatics<T> {
  static shared = 1;
  own = 2;
  read(value: T) {
    return [shared, own, value, missingThree];
  }
}

class GenericIndex<T> {
  static [key: string]: unknown;
  read(value: T) {
    return [value, missingFour];
  }
}

class FunctionMembers {
  read() {
    return [length, call, missingFive];
  }
}
