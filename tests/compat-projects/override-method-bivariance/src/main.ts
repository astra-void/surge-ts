abstract class Base {
  handle(value: string): void {}
  abstract visit(value: string): void;
  callback: (value: string) => void = () => {};
}

export class Narrower extends Base {
  handle(value: "exact"): void {}
  visit(value: "exact"): void {}
  callback: (value: "exact") => void = () => {};
}
