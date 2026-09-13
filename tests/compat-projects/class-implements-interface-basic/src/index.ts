interface Shape {
  size: number;
  render(): void;
}

interface Named {
  label?: string;
}

type Sized = { width: number };

export class Missing implements Shape {}

export class Partial implements Shape {
  size = 1;
}

export class MissingAlias implements Sized {}

export class Two implements Shape, Sized {}

export abstract class StillChecked implements Shape {}

export class Complete implements Shape, Named, Sized {
  label = 'x';
  size = 1;
  width = 2;
  render(): void {}
}

class Base {
  size = 1;
}

export class Inherited extends Base implements Shape {
  render(): void {}
}

export class ByAccessorAndParameter implements Shape {
  get size(): number {
    return 1;
  }
  render(): void {}
}

export class OptionalOnly implements Named {}
