export {};

declare declare const twice: number;
class DeclaredConstructor {
  declare constructor() {}
}
interface TypeMember {
  public a: string;
}
interface IndexModifier {
  public [key: string]: number;
}
function parameterModifier(static x: number) {}
type TypeParameterModifier<public X> = X;
class ConstMember {
  const x = 1;
}
function restOptional(...x?: number[]) {}
declare namespace UsingInAmbient {
  using resource: any;
}
declare namespace AwaitUsingInAmbient {
  await using resource: any;
}
