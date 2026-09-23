namespace M {
  export default 1;
}
namespace P {
  export default function f() {}
}
declare namespace Q {
  export default function g(): void;
}
namespace R {
  export const ok = 1;
}
declare module "amb" {
  export default function h(): void;
}
export default 2;
