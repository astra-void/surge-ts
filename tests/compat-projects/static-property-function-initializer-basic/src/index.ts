class Schema {
  static create = (label?: string): Schema => new Schema();
  static parse = function (input: number): string {
    return String(input);
  };
  static make<T>(value: T): T[] {
    return [value];
  }
}

const created: number = Schema.create;
const parsed: number = Schema.parse(1);
Schema.create(42);
Schema.parse("1");
const made: string[] = Schema.make(1);
export {};
