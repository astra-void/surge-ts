interface NumberBase {
  [index: number]: number;
  length: number;
}
interface NumberDerived extends NumberBase {
  extra: string;
}
interface ArrayDerived extends Array<string> {
  tag: number;
}
declare const numberDerived: NumberDerived;
declare const arrayDerived: ArrayDerived;
declare function strings(parts: TemplateStringsArray): string;

export const inheritedRead: string = numberDerived[0];
export const inheritedArrayRead: number = arrayDerived[0];
export const toBase: NumberBase = numberDerived;
export const toIndexed: { [key: number]: number } = numberDerived;
export const wrongValue: { [key: number]: string } = numberDerived;
export function firstPart(parts: TemplateStringsArray) {
  return strings(parts) + parts[0];
}

interface Shape {
  hello: string;
  world: number;
}
interface StringToAny {
  [key: string]: any;
}
interface NumberToAny {
  [index: number]: any;
}
type ShapeLiteral = { hello: string; world: number };
declare let shape: Shape;
declare let shapeLiteral: ShapeLiteral;
declare let stringToAny: StringToAny;
declare let numberToAny: NumberToAny;
declare let both: StringToAny & NumberToAny;

stringToAny = shape;
numberToAny = shape;
numberToAny = shapeLiteral;
shape = stringToAny;
shape = numberToAny;
shape = both;
