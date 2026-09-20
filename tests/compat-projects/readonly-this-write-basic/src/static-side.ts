export class Registry {
  static readonly limit: number = 1;
  static count = 0;

  static get size() {
    return Registry.count;
  }

  static get capacity() {
    return 1;
  }
  static set capacity(value: number) {
    Registry.count = value;
  }

  static reset() {
    this.limit = 2;
    this.count = 0;
  }
}

Registry.limit = 3;
Registry.size = 4;
Registry.capacity = 5;
Registry.count = 6;
