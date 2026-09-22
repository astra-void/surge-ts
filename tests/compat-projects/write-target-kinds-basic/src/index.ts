export {};

enum Colors {
  Red,
}
class Shape {}
function helper() {}
namespace Space {
  export const value = 1;
}
const fixed = 1;
const Anonymous = class {};

Colors = 1 as any;
Shape = 1 as any;
helper = 1 as any;
Space = 1 as any;
Colors++;
helper++;
fixed = 2;
Anonymous = 1 as any;

function shadows() {
  let Colors = 1;
  Colors = 2;
  const Shape = 1;
  Shape = 2;
}
