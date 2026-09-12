declare module "events-like" {
  class Emitter {
    on(): this;
    static defaultMaxListeners: number;
  }
  namespace Emitter {
    export const captureRejections: boolean;
    export { Emitter as EventEmitter };
  }
  export = Emitter;
}
declare module "node:events-like" {
  export * from "events-like";
}
declare module "stream-like" {
  import { EventEmitter } from "node:events-like";
  class Stream extends EventEmitter {
    pipe(): void;
  }
  namespace Stream {
    const streamMarker: number;
  }
  export = Stream;
}
declare module "named-like" {
  export class Base {
    on(): this;
    static shared: number;
  }
  export namespace Base {
    export const limit: number;
  }
  export class Derived extends Base {
    pipe(): void;
  }
}
