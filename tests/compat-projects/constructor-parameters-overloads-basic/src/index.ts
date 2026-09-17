interface Widget {
  new (): object;
  new (size: number): object;
}
interface Callable {
  (): string;
  new (value: boolean): object;
}

// `infer` against overloaded construct signatures reads the last one.
const lastOverload: ConstructorParameters<Widget>[0] = "large";
// A value that is both callable and constructible matches its construct side.
const constructSide: ConstructorParameters<Callable>[0] = "not a boolean";
const dateArgument: ConstructorParameters<typeof Date>[0] = true;
const dateOk: ConstructorParameters<typeof Date>[0] = "2024-01-01";
