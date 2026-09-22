export {};

class Klass {}
Klass();
interface Constructable {
  new (): object;
}
declare const constructable: Constructable;
constructable();

undefined = 1 as any;

class StaticBlocks {
  public static {}
  async static {}
}

class AsyncConstructor {
  async constructor() {}
}

{
  export const inBlock = 1;
}
function body() {
  declare const inBody: number;
}
export declare const topLevel: number;
declare namespace Ambient {
  const member: number;
}
