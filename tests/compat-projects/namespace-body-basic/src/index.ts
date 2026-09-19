export {};

const x = 1;
namespace Shadow {
  const x = "inner";
  export const y: number = x;
}

namespace M {
  export var m = 0;
  export namespace N {
    export var n = 1;
  }
}
namespace M {
  var y = m;
  export namespace N {
    var z = n + y;
  }
}
const deep: number = M.N.n;

namespace Q {
  export interface I3 { zeep: string }
  export namespace K2 {
    interface I4 { z: string }
    declare var v1: I4;
    var v2: I3 = v1;
    var v3: () => I3 = v1;
  }
  function later(): number {
    return value;
  }
  const value = "s";
  export class C {
    m(): number {
      return null;
    }
  }
}

declare namespace Ambient {
  class Holder {
    readonly value: string;
  }
}
