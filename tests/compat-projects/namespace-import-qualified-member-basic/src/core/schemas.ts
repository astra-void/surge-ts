export interface Schema<Output = unknown, Input = unknown> {
  _output: Output;
  _input: Input;
}

export interface SomeType {
  marker: string;
}
