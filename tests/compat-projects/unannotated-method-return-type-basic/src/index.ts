interface Shape {
  area(): number;
}

class Label implements Shape {
  area() {
    return "wide";
  }
}

class Base {
  measure(): number {
    return 1;
  }
}

class Derived extends Base {
  measure() {
    return "long";
  }
}

class Counter {
  next() {
    return 1;
  }
  describe(verbose: boolean) {
    if (verbose) {
      return "counter";
    }
    return 0;
  }
  reset() {
    this.touched = true;
  }
  touched = false;
}

const next: string = new Counter().next();
const described: boolean = new Counter().describe(true);
const reset: number = new Counter().reset();
const nextOk: number = new Counter().next();
