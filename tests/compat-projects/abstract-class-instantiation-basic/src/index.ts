export abstract class Shape {
  abstract area(): number;
}

export class Circle extends Shape {
  area(): number {
    return 1;
  }
}

export const wrong = new Shape();
export const right = new Circle();

abstract class Generic<T> {
  abstract handle(value: T): void;
}

export const wrongGeneric = new Generic<string>();

declare abstract class Ambient {
  abstract run(): void;
}

export const wrongAmbient = new Ambient();
