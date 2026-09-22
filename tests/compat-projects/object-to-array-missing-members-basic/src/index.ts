interface Shape {
  side: number;
}
class Holder {
  held = 1;
}
declare const shape: Shape;
declare const holder: Holder;
declare function takesList(list: number[]): void;

export const fromLiteral: string[] = { one: 1 };
export const fromInterface: Shape[] = shape;
export const fromClass: unknown[] = holder;
takesList(shape);

export const arrayLike: number[] = { length: 0, pop: () => 1 };
export const wrongElement: number[] = ["a"];
