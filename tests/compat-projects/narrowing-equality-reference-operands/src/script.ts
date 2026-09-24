interface I1 { p1: number }
interface I2 extends I1 { p2: number }
interface I3 { p3: number }

var source = { p1: 10, p2: 20, p3: 30 };
var y: I1 & I3 = source;
var z: I2 = source;

if (y === z || z === y) {
} else if (y !== z || z !== y) {
} else if (y == z || z == y) {
} else if (y != z || z != y) {
}
