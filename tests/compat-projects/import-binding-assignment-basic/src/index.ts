import Widget, { counter, increment } from "./source";
import * as source from "./source";

counter = 1;
increment = () => {};
Widget = class {};
source = {} as typeof source;

function update() {
  counter = 2;
}

function shadowed() {
  let counter = 0;
  counter = 3;
}
