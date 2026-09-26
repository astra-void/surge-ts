class Point {
  x = 0;
  constructor(flag: boolean) {
    if (flag) {
      return 1;
    }
  }
}

class Empty {
  constructor() {
    return 1;
  }
}

class Same {
  y = 1;
  constructor(other?: Same) {
    if (other) {
      return other;
    }
    return this;
  }
}

class Bare {
  z = 1;
  constructor() {
    return;
  }
}

class Nested {
  v = 1;
  constructor() {
    const read = () => {
      return 1;
    };
    function write(): string {
      return "w";
    }
    read();
    write();
  }
}

class Choice {
  w = 1;
  constructor(flag: boolean) {
    return flag
      ? 1
      : "two";
  }
}
