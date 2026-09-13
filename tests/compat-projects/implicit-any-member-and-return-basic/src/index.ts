export class Widget {
  label;
  size: number = 1;
  initialized = 2;

  render();
  render(): void {}
}

export interface Shape {
  label;
  render();
  sized: number;
  typed(): void;
}

export type Literal = {
  label;
  render();
};

export declare function ambient();
