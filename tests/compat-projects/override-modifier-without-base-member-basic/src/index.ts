class Base {
  run() {}
  size = 1;
  get label() {
    return "base";
  }
}

class Standalone {
  override run() {}
}

interface Runner {
  run(): void;
}
class Implementer implements Runner {
  override run() {}
}

class Misnamed extends Base {
  override walk() {}
  override count = 2;
}

class Middle extends Base {}
class Leaf extends Middle {
  override run() {}
  override size = 3;
  override get label() {
    return "leaf";
  }
  override jump() {}
}

abstract class Shape {
  abstract area(): number;
}
class Square extends Shape {
  override area() {
    return 1;
  }
}
