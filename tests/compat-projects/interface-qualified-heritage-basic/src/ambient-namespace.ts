declare namespace Ambient {
  interface Config {
    level: number;
  }

  interface Wrapper<T> {
    unwrap(): T;
  }

  namespace Deep {
    interface Tagged {
      tag: string;
    }
  }

  class Emitter {
    emit(event: string): void;
  }
}

export interface DerivedConfig extends Ambient.Config {
  label: string;
}

export interface DerivedWrapper extends Ambient.Wrapper<string> {
  cached: boolean;
}

export interface DerivedTagged extends Ambient.Deep.Tagged {
  order: number;
}

export class DerivedEmitter extends Ambient.Emitter {
  name = "derived";
}
