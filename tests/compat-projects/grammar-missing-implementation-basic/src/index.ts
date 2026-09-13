export function overloaded(value: string): void;
export function overloaded(value: number): void;

export class Widget {
  render(value: string): void;
  render(value: number): void;

  constructor(value: string);
}

export class Implemented {
  render(value: string): void;
  render(value: number): void;
  render(value: string | number): void {
    void value;
  }

  constructor(value: string) {
    void value;
  }
}

export abstract class WithAbstract {
  abstract render(value: string): void;
}

export declare class Ambient {
  render(value: string): void;
}

declare function ambientFunction(value: string): void;

export { ambientFunction };
