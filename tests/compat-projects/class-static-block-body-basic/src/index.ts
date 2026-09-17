export class Registry {
  static items: string[] = [];
  static count = 0;
  static {
    const bad: string = 3;
    this.items.push("a");
    this.count = this.items.length;
    this.missing = 1;
    Registry.items.push(2);
  }
  value = 1;
}
