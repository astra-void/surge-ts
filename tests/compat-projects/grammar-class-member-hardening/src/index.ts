class Base {
  constructor(public value: number) {}
}

export class MissingSuper extends Base {
  constructor() {}
}

export class CallsSuper extends Base {
  constructor(flag: boolean) {
    if (flag) {
      super(1);
    } else {
      super(2);
    }
  }
}

export class TwoConstructors {
  constructor(value: string) {
    void value;
  }
  constructor(value: number) {
    void value;
  }
}

export class TwoImplementations {
  render(): void {}
  render(): void {}
}

export class DuplicateProperty {
  label = 1;
  label = 2;
}

export class PropertyAndMethod {
  handler = 1;
  handler(): void {}
}

export class StaticAndInstance {
  static shared = 1;
  shared = 2;
}

export class Overloaded {
  render(value: string): void;
  render(value: number): void;
  render(value: string | number): void {
    void value;
  }

  get size(): number {
    return 1;
  }
  set size(next: number) {
    void next;
  }
}

export interface DuplicateMember {
  label: number;
  label: number;
}

export interface OverloadedMember {
  render(value: string): void;
  render(value: number): void;
  size: number;
}

export const duplicateMethods = { render() {}, render() {} };
