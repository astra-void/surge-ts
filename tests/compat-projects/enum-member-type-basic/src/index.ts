export enum Color {
  Red = 1,
  Green = 2,
}

export enum Label {
  Wide = "wide",
  Tall = "tall",
}

type RedOnly = Color.Red;
declare const red: RedOnly;
export const wrongRed: string = red;

interface Wide {
  readonly kind: Label.Wide;
  width: number;
}
interface Tall {
  readonly kind: Label.Tall;
  height: number;
}
declare const shape: Wide | Tall;
export const wrongKind: number = shape.kind;

export function area(value: Wide | Tall) {
  return value.kind === Label.Wide ? value.width : value.height;
}
