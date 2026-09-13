abstract class One {
  abstract run(): void;
}

export class MissingOne extends One {}

abstract class Two {
  abstract first(): void;
  abstract second: number;
}

export class MissingTwo extends Two {}

abstract class Six {
  abstract a(): void;
  abstract b(): void;
  abstract c(): void;
  abstract d(): void;
  abstract e(): void;
  abstract f(): void;
}

export class MissingSix extends Six {}

export abstract class StillAbstract extends One {}

abstract class Middle extends One {
  run(): void {}
}

export class Implemented extends Middle {}

abstract class Members {
  abstract size: number;
  abstract get label(): string;
  abstract run(): void;
}

export class AllImplemented extends Members {
  abstract_placeholder = 0;
  get label(): string {
    return '';
  }
  run(): void {}
  constructor(public size: number) {
    super();
  }
}

abstract class Generic<T> {
  abstract handle(value: T): void;
}

export class MissingGeneric extends Generic<string> {}
