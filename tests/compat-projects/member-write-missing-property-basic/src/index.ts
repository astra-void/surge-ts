interface Point {
  x: number;
}
declare const point: Point;
point.y = 1;
point.x.z = 2;

class Box {
  value = 1;
}
new Box().missing = 1;
Box.staticMissing = 1;

function local(receiver: Point) {
  receiver.w = 1;
}

function declaredFunction() {}
declaredFunction.expando = 1;

const constArrow = () => {};
constArrow.expando = "x";

let letFunction = function () {};
letFunction.notExpando = 1;
