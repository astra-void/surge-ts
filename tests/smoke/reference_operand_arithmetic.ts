type Out<T> = T extends { value: infer V } ? V : unknown;

interface Wrapper {
  transform(fn: (value: Out<{ value: string }>) => Out<{ value: string }>): void;
  loose(fn: (value: Out<any>) => Out<any>): void;
}

declare const wrapper: Wrapper;

export function run(): void {
  wrapper.transform((value) => value + "a");
  wrapper.loose((value) => value + "a");
}
